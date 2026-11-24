use std::collections::BTreeSet;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::BinaryPackageRule;

pub struct PostrmDoesNotCallUpdatercDForInitDScript;

declare_rule! {
    /// `postinst` registers an init.d script with `update-rc.d` (a non-`remove`
    /// call) but `postrm` never calls `update-rc.d` for it.
    ///
    /// Each name registered in `postinst` is reported once, unless `postrm`
    /// calls `update-rc.d` for it (any action, `remove` included). The init-d
    /// check ignores comments and the `if [ -x .../update-rc.d ]` guard, and so
    /// do we. A `postinst` call counts only when it starts a line or follows a
    /// `;`, as in lintian. The extra is `etc/init.d/<name>`, in name order. The
    /// Location is `postrm` with no line. Skipped for the `initscripts` and
    /// `sysvinit` packages. A symlink `postrm` or `postinst` is read through its
    /// target in the same directory, as lintian reads it.
    PostrmDoesNotCallUpdatercDForInitDScript,
    code = "LN1098",
    tag = "postrm-does-not-call-updaterc.d-for-init.d-script",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PostrmDoesNotCallUpdatercDForInitDScript {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if matches!(ctx.package_name(), "initscripts" | "sysvinit") {
            return;
        }
        let Some(control) = ctx.control_files() else {
            return;
        };
        let missing = unremoved_from_postrm(control);
        for name in missing {
            ctx.diagnostic(
                Diagnostic::error(format!("etc/init.d/{name}")).with_location(Location {
                    path: "postrm".into(),
                    line: None,
                    column: None,
                }),
            );
        }
    }
}

static EXCLUDE_UPDATE_RC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"if\s+\[\s+-x\s+\S*update-rc\.d").expect("valid regex"));
/// `postinst` form: anchored, and it needs an action after the name.
static POSTINST_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:.+;|^\s*system[\s(']+)?\s*update-rc\.d\s+(?:-\S+\s*)*([\w.+][\w\-.+]*)\s+(\w+)",
    )
    .expect("valid regex")
});
/// `postrm` form: unanchored, and the action is not needed.
static POSTRM_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"update-rc\.d\s+(?:-\S+\s*)*([\w.+][\w\-.+]*)").expect("valid regex")
});

/// Names `postinst` registers that `postrm` does not call `update-rc.d` for.
fn unremoved_from_postrm(control: &FileTree) -> BTreeSet<String> {
    let called = called_in_postrm(control);
    registered_in_postinst(control)
        .into_iter()
        .filter(|name| !called.contains(name))
        .collect()
}

/// Names with a non-`remove` `update-rc.d` call in `postinst`.
fn registered_in_postinst(control: &FileTree) -> BTreeSet<String> {
    code_lines(control, "postinst")
        .into_iter()
        .filter_map(|(_, code)| {
            let caps = POSTINST_CALL.captures(&code)?;
            (&caps[2] != "remove").then(|| caps[1].to_string())
        })
        .collect()
}

/// Names with any `update-rc.d` call in `postrm`.
fn called_in_postrm(control: &FileTree) -> BTreeSet<String> {
    code_lines(control, "postrm")
        .into_iter()
        .filter_map(|(_, code)| Some(POSTRM_CALL.captures(&code)?[1].to_string()))
        .collect()
}

/// Lines of a maintainer script as the init-d check reads them: the 1-based
/// number, with the guard lines dropped and everything from `#` cut off.
fn code_lines(control: &FileTree, name: &str) -> Vec<(u32, String)> {
    let Some(text) = script_text(control, name) else {
        return Vec::new();
    };
    text.lines()
        .enumerate()
        .filter(|(_, line)| !EXCLUDE_UPDATE_RC.is_match(line))
        .map(|(index, line)| {
            let code = match line.find('#') {
                Some(at) => &line[..at],
                None => line,
            };
            let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
            (number, code.to_string())
        })
        .collect()
}

/// The text of a control file that is a regular file, a hardlink, or a
/// symlink to one in the same directory.
fn script_text(control: &FileTree, name: &str) -> Option<String> {
    let mut entry = control.get(name)?;
    if entry.is_symlink() {
        let target = entry.link_target()?;
        if target.is_absolute() {
            return None;
        }
        let parent = Path::new(name).parent().unwrap_or(Path::new(""));
        entry = control.get(parent.join(target))?;
    }
    if !entry.is_file() {
        return None;
    }
    let bytes = control.read(entry).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(postinst: Option<&str>, postrm: Option<&str>) -> BinaryPackageCase {
        let mut case = BinaryPackageCase::from(Vec::<(String, TestFile)>::new());
        if let Some(text) = postinst {
            case = case.control_file("postinst", TestFile::new(text));
        }
        if let Some(text) = postrm {
            case = case.control_file("postrm", TestFile::new(text));
        }
        case
    }

    #[test]
    fn binary() {
        let pass = vec![
            case(None, None),
            case(
                Some("update-rc.d foo defaults\n"),
                Some("update-rc.d foo remove\n"),
            ),
            case(
                Some("update-rc.d foo defaults\n"),
                Some("update-rc.d -f foo remove\n"),
            ),
            case(
                Some("update-rc.d foo remove\n"),
                Some("update-rc.d bar remove\n"),
            ),
            case(
                Some("# update-rc.d foo defaults\n"),
                Some("update-rc.d bar remove\n"),
            ),
            case(
                Some("if [ -x \"/usr/sbin/update-rc.d\" ]; then update-rc.d foo defaults; fi\n"),
                None,
            ),
            case(Some("if true; then update-rc.d foo defaults; fi\n"), None),
            case(
                Some("update-rc.d foo defaults\n"),
                Some("echo update-rc.d foo\n"),
            ),
            case(Some("update-rc.d foo defaults\n"), None).package("initscripts"),
            case(Some("update-rc.d foo defaults\n"), None).package("sysvinit"),
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new())
                .control_file("postinst", TestFile::symlink("postinst.real"))
                .control_file("postinst.real", TestFile::new("update-rc.d foo defaults\n"))
                .control_file("postrm", TestFile::new("update-rc.d foo remove\n")),
        ];
        let fail = vec![
            case(Some("update-rc.d foo defaults\n"), None),
            case(
                Some("update-rc.d foo defaults\n"),
                Some("update-rc.d bar remove\n"),
            ),
            case(
                Some("update-rc.d foo defaults\nupdate-rc.d bar defaults\n"),
                Some("update-rc.d bar remove\n"),
            ),
            case(
                Some("update-rc.d foo defaults\n"),
                Some("# update-rc.d foo remove\n"),
            ),
            case(Some("update-rc.d foo defaults\n"), None).udeb(),
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new())
                .control_file("postinst", TestFile::symlink("postinst.real"))
                .control_file("postinst.real", TestFile::new("update-rc.d foo defaults\n")),
        ];
        BinaryPackageTester::new(PostrmDoesNotCallUpdatercDForInitDScript, pass, fail)
            .test_and_snapshot();
    }
}
