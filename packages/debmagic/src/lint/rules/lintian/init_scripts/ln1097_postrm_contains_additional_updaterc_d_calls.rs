use std::collections::BTreeSet;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::BinaryPackageRule;

pub struct PostrmContainsAdditionalUpdatercDCalls;

declare_rule! {
    /// `postrm` calls `update-rc.d` for an init.d script that `postinst` does
    /// not register.
    ///
    /// Each name called in `postrm` (`remove` calls included) is reported once,
    /// unless `postinst` has a non-`remove` `update-rc.d` call for it. The
    /// init-d check ignores comments and the `if [ -x .../update-rc.d ]` guard,
    /// and so do we. A `postinst` call counts only when it starts a line or
    /// follows a `;`, as in lintian. The extra is `etc/init.d/<name>`, in name
    /// order. The Location is `postrm` with no line. Skipped for the
    /// `initscripts` and `sysvinit` packages. A symlink `postrm` or `postinst`
    /// is read through its target in the same directory, as lintian reads it.
    PostrmContainsAdditionalUpdatercDCalls,
    code = "LN1097",
    tag = "postrm-contains-additional-updaterc.d-calls",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PostrmContainsAdditionalUpdatercDCalls {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if matches!(ctx.package_name(), "initscripts" | "sysvinit") {
            return;
        }
        let Some(control) = ctx.control_files() else {
            return;
        };
        let extra = additional_postrm_calls(control);
        for name in extra {
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

/// Names `postrm` calls `update-rc.d` for, minus those `postinst` registers.
fn additional_postrm_calls(control: &FileTree) -> BTreeSet<String> {
    let registered = postinst_registered(control);
    called_in_postrm(control)
        .into_iter()
        .filter(|name| !registered.contains(name))
        .collect()
}

/// Names with a non-`remove` `update-rc.d` call in `postinst`.
fn postinst_registered(control: &FileTree) -> BTreeSet<String> {
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
                Some("update-rc.d bar defaults\n"),
                Some("update-rc.d bar remove\n"),
            ),
            case(
                Some("update-rc.d bar defaults\n"),
                Some("update-rc.d -f bar remove\n"),
            ),
            case(
                Some("update-rc.d bar defaults\n"),
                Some("# update-rc.d baz remove\n"),
            ),
            case(
                Some("update-rc.d bar defaults\n"),
                Some("if [ -x \"/usr/sbin/update-rc.d\" ]; then update-rc.d baz remove; fi\n"),
            ),
            case(
                Some("update-rc.d bar defaults\n"),
                Some("update-rc.d bar remove\nupdate-rc.d bar remove\n"),
            ),
            case(
                Some("update-rc.d bar defaults\n"),
                Some("echo update-rc.d\n"),
            ),
            case(None, Some("update-rc.d bar remove\n")).package("initscripts"),
            case(None, Some("update-rc.d bar remove\n")).package("sysvinit"),
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new())
                .control_file("postrm", TestFile::symlink("postrm.real")),
        ];
        let fail = vec![
            case(None, Some("update-rc.d bar remove\n")),
            case(
                Some("update-rc.d bar defaults\n"),
                Some("update-rc.d baz remove\n"),
            ),
            case(
                Some("update-rc.d bar remove\n"),
                Some("update-rc.d bar remove\n"),
            ),
            case(
                Some("update-rc.d bar defaults\n"),
                Some("update-rc.d bar remove\nupdate-rc.d baz remove\n"),
            ),
            case(
                Some("if true; then update-rc.d bar defaults; fi\n"),
                Some("update-rc.d bar remove\n"),
            ),
            case(None, Some("update-rc.d bar remove\n")).udeb(),
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new())
                .control_file("postrm", TestFile::symlink("postrm.real"))
                .control_file("postrm.real", TestFile::new("update-rc.d bar remove\n")),
        ];
        BinaryPackageTester::new(PostrmContainsAdditionalUpdatercDCalls, pass, fail)
            .test_and_snapshot();
    }
}
