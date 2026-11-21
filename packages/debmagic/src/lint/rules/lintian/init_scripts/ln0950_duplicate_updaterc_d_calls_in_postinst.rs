use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

pub struct DuplicateUpdatercDCallsInPostinst;

declare_rule! {
    /// `postinst` calls `update-rc.d` twice for the same init script name.
    ///
    /// Only the second non-`remove` call for a name is reported, at its line.
    /// Lines that the init-d check ignores are skipped the same way: the
    /// `if [ -x .../update-rc.d ]` guard and everything after `#`. The extra is
    /// the script name. The Location is `postinst` at that line.
    DuplicateUpdatercDCallsInPostinst,
    code = "LN0950",
    tag = "duplicate-updaterc.d-calls-in-postinst",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DuplicateUpdatercDCallsInPostinst {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(control) = ctx.control_files() else {
            return;
        };
        let duplicates = duplicate_calls(control);
        for (line, name) in duplicates {
            ctx.diagnostic(Diagnostic::error(name).with_location(Location {
                path: "postinst".into(),
                line: Some(line),
                column: None,
            }));
        }
    }
}

static EXCLUDE_UPDATE_RC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"if\s+\[\s+-x\s+\S*update-rc\.d").expect("valid regex"));
static UPDATE_RC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:.+;|^\s*system[\s(']+)?\s*update-rc\.d\s+(?:-\S+\s*)*([A-Za-z0-9_.+][A-Za-z0-9_.+-]*)\s+([A-Za-z0-9_]+)",
    )
    .expect("valid regex")
});

/// `(line, name)` for each second non-`remove` `update-rc.d` call of a name.
fn duplicate_calls(control: &FileTree) -> Vec<(u32, String)> {
    let Some(entry) = control.get(Path::new("postinst")) else {
        return Vec::new();
    };
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return Vec::new();
    }
    let Ok(bytes) = control.read(entry) else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&bytes);
    let mut seen: HashMap<String, u32> = HashMap::new();
    let mut hits = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if EXCLUDE_UPDATE_RC.is_match(line) {
            continue;
        }
        let code = match line.find('#') {
            Some(at) => &line[..at],
            None => line,
        };
        let Some(caps) = UPDATE_RC.captures(code) else {
            continue;
        };
        if &caps[2] == "remove" {
            continue;
        }
        let count = seen.entry(caps[1].to_string()).or_insert(0);
        *count += 1;
        if *count == 2 {
            let line_no = u32::try_from(index + 1).unwrap_or(u32::MAX);
            hits.push((line_no, caps[1].to_string()));
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(postinst: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(String, TestFile)>::new())
            .control_file("postinst", TestFile::new(postinst))
    }

    #[test]
    fn binary() {
        let pass = vec![
            case("update-rc.d foo defaults\n"),
            case("update-rc.d foo defaults\nupdate-rc.d bar defaults\n"),
            case("update-rc.d foo remove\nupdate-rc.d foo defaults\n"),
            case("update-rc.d foo defaults\nupdate-rc.d foo remove\n"),
            case("# update-rc.d foo defaults\nupdate-rc.d foo defaults\n"),
            case(
                "if [ -x \"/usr/sbin/update-rc.d\" ]; then update-rc.d foo defaults; fi\n\
                 update-rc.d foo defaults\n",
            ),
            case("update-rc.d foo defaults\n").udeb(),
        ];
        let fail = vec![
            case("update-rc.d foo defaults\nupdate-rc.d foo defaults\n").udeb(),
            case("update-rc.d foo defaults\nupdate-rc.d foo defaults\n"),
            case(
                "update-rc.d foo defaults\nupdate-rc.d foo remove\nupdate-rc.d foo start 20 2 3 .\n",
            ),
            case("update-rc.d foo defaults\nupdate-rc.d foo defaults\nupdate-rc.d foo defaults\n"),
            case(
                "update-rc.d foo defaults >/dev/null\nupdate-rc.d bar defaults\nupdate-rc.d foo defaults\n",
            ),
        ];
        BinaryPackageTester::new(DuplicateUpdatercDCallsInPostinst, pass, fail).test_and_snapshot();
    }
}
