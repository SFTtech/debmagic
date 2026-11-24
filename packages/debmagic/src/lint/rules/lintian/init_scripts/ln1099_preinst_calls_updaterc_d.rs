use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::BinaryPackageRule;

pub struct PreinstCallsUpdatercD;

declare_rule! {
    /// `preinst` calls `update-rc.d` with an action other than `remove`.
    ///
    /// Each line with such a call is reported, so a name called twice is
    /// reported twice. The extra is the script name and the action, separated
    /// by a space (`bar defaults`). The Location is `preinst` at that line. The
    /// init-d check ignores comments and the `if [ -x .../update-rc.d ]` guard,
    /// and so do we. A call with no action after the name is not a call. Skipped
    /// for the `initscripts` and `sysvinit` packages, as the whole init-d check
    /// is. A symlink `preinst` is read through its target in the same
    /// directory, as lintian reads it.
    PreinstCallsUpdatercD,
    code = "LN1099",
    tag = "preinst-calls-updaterc.d",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PreinstCallsUpdatercD {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if matches!(ctx.package_name(), "initscripts" | "sysvinit") {
            return;
        }
        let Some(control) = ctx.control_files() else {
            return;
        };
        let calls = calls(control);
        for (line, extra) in calls {
            ctx.diagnostic(Diagnostic::error(extra).with_location(Location {
                path: "preinst".into(),
                line: Some(line),
                column: None,
            }));
        }
    }
}

static EXCLUDE_UPDATE_RC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"if\s+\[\s+-x\s+\S*update-rc\.d").expect("valid regex"));
/// `preinst` form: unanchored, and the action is required.
static PREINST_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"update-rc\.d\s+(?:-\S+\s*)*([\w.+][\w\-.+]*)\s+(\w+)").expect("valid regex")
});

/// `(line, "<name> <action>")` for each non-`remove` call in `preinst`.
fn calls(control: &FileTree) -> Vec<(u32, String)> {
    code_lines(control, "preinst")
        .into_iter()
        .filter_map(|(line, code)| {
            let caps = PREINST_CALL.captures(&code)?;
            (&caps[2] != "remove").then(|| (line, format!("{} {}", &caps[1], &caps[2])))
        })
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

    fn case(preinst: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(String, TestFile)>::new())
            .control_file("preinst", TestFile::new(preinst))
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            case("update-rc.d bar remove\n"),
            case("update-rc.d -f bar remove\n"),
            case("# update-rc.d bar defaults\n"),
            case("update-rc.d bar\n"),
            case("if [ -x \"/usr/sbin/update-rc.d\" ]; then update-rc.d bar defaults; fi\n"),
            case("echo update-rc.d\n"),
            case("update-rc.d bar defaults\n").package("sysvinit"),
            case("update-rc.d bar defaults\n").package("initscripts"),
        ];
        let fail = vec![
            case("update-rc.d bar defaults\n"),
            case("update-rc.d -f bar stop 20 0 1 6 .\n"),
            case(
                "update-rc.d bar defaults\nupdate-rc.d bar remove\nupdate-rc.d baz start 20 2 3 .\n",
            ),
            case("update-rc.d bar defaults\nupdate-rc.d bar defaults\n"),
            case("update-rc.d bar defaults >/dev/null # comment\n").udeb(),
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new())
                .control_file("preinst", TestFile::symlink("preinst.real"))
                .control_file("preinst.real", TestFile::new("update-rc.d bar defaults\n")),
        ];
        BinaryPackageTester::new(PreinstCallsUpdatercD, pass, fail).test_and_snapshot();
    }
}
