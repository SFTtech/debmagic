use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct OutputOfUpdatercDNotRedirectedToDevNull;

declare_rule! {
    /// An `update-rc.d` call in `postinst` or `postrm` does not redirect its
    /// output to `/dev/null`.
    ///
    /// `update-rc.d` is chatty, so each call in the maintainer script should end
    /// with `>/dev/null`. Reported as an info hint per call, with the init
    /// script name as Extra and `postinst` or `postrm` at that line as the
    /// Location. The check is the init-d check's: the comment after `#` and the
    /// `if [ -x .../update-rc.d ]` guard are ignored, and so is a second call
    /// for the same name in the same script (the duplicate Tag reports that).
    /// In `postinst`, `remove` calls are skipped, as the init-d check skips
    /// them. In `postrm`, any call counts, including `remove`. Only regular
    /// files and hardlinks are read. Udebs are checked too.
    OutputOfUpdatercDNotRedirectedToDevNull,
    code = "LN1743",
    tag = "output-of-updaterc.d-not-redirected-to-dev-null",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for OutputOfUpdatercDNotRedirectedToDevNull {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(control) = ctx.control_files() else {
            return;
        };
        let hits: Vec<(&str, u32, String)> = ["postinst", "postrm"]
            .into_iter()
            .flat_map(|script| {
                unredirected_calls(control, script)
                    .into_iter()
                    .map(move |(line, name)| (script, line, name))
            })
            .collect();
        for (script, line, name) in hits {
            ctx.diagnostic(
                Diagnostic::warn(name)
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path: script.into(),
                        line: Some(line),
                        column: None,
                    }),
            );
        }
    }
}

static EXCLUDE_UPDATE_RC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"if\s+\[\s+-x\s+\S*update-rc\.d").expect("valid regex"));
/// `postinst` calls, with the action. The init-d check's anchored pattern.
static POSTINST_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:.+;|^\s*system[\s(']+)?\s*update-rc\.d\s+(?:-\S+\s*)*([A-Za-z0-9_.+][A-Za-z0-9_.+-]*)\s+([A-Za-z0-9_]+)",
    )
    .expect("valid regex")
});
/// `postrm` calls: any `update-rc.d` line that names a script.
static POSTRM_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"update-rc\.d\s+(?:-\S+\s*)*([A-Za-z0-9_.+][A-Za-z0-9_.+-]*)").expect("valid regex")
});
static REDIRECTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r">\s*/dev/null").expect("valid regex"));

/// `(line, name)` for each call in `script` whose line does not redirect to
/// `/dev/null`, except a second call for the same name.
fn unredirected_calls(control: &FileTree, script: &str) -> Vec<(u32, String)> {
    let Some(entry) = control.get(Path::new(script)) else {
        return Vec::new();
    };
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return Vec::new();
    }
    let Ok(bytes) = control.read(entry) else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&bytes);
    let is_postinst = script == "postinst";
    let mut counts: HashMap<String, u32> = HashMap::new();
    let mut hits = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if EXCLUDE_UPDATE_RC.is_match(line) {
            continue;
        }
        let code = match line.find('#') {
            Some(at) => &line[..at],
            None => line,
        };
        let name = if is_postinst {
            let Some(caps) = POSTINST_CALL.captures(code) else {
                continue;
            };
            if &caps[2] == "remove" {
                continue;
            }
            caps[1].to_string()
        } else {
            let Some(caps) = POSTRM_CALL.captures(code) else {
                continue;
            };
            caps[1].to_string()
        };
        let count = counts.entry(name.clone()).or_insert(0);
        *count += 1;
        // The second call for a name is the duplicate Tag's, and is not checked.
        if *count == 2 {
            continue;
        }
        if !REDIRECTED.is_match(code) {
            let line_no = u32::try_from(index + 1).unwrap_or(u32::MAX);
            hits.push((line_no, name));
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn postinst(content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(String, TestFile)>::new())
            .control_file("postinst", TestFile::new(content))
    }

    fn postrm(content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(String, TestFile)>::new())
            .control_file("postrm", TestFile::new(content))
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            postinst("update-rc.d foo defaults >/dev/null\n"),
            // A remove call in postinst is skipped.
            postinst("update-rc.d foo remove\n"),
            // The second call for a name is the duplicate Tag's.
            postinst("update-rc.d foo defaults >/dev/null\nupdate-rc.d foo defaults\n"),
            // The guarded call is ignored.
            postinst("if [ -x /usr/sbin/update-rc.d ]; then update-rc.d foo defaults; fi\n"),
            postrm("update-rc.d foo remove >/dev/null\n"),
            postrm("update-rc.d foo remove >/dev/null\nupdate-rc.d foo remove >/dev/null\n"),
            postinst("update-rc.d foo defaults >/dev/null\n").udeb(),
        ];
        let fail = vec![
            postinst("update-rc.d foo defaults\n"),
            // The third call is checked, the second is not.
            postinst(
                "update-rc.d foo defaults >/dev/null\nupdate-rc.d foo defaults\nupdate-rc.d foo defaults\n",
            ),
            postinst("update-rc.d foo defaults\n").udeb(),
            postrm("update-rc.d bar remove\n"),
            // A comment does not count as a redirect.
            postinst("update-rc.d foo defaults # >/dev/null\n"),
        ];
        BinaryPackageTester::new(OutputOfUpdatercDNotRedirectedToDevNull, pass, fail)
            .test_and_snapshot();
    }
}
