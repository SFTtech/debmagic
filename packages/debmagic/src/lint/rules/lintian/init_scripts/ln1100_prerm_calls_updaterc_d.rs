use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::BinaryPackageRule;

pub struct PrermCallsUpdatercD;

declare_rule! {
    /// `prerm` calls `update-rc.d`, for any action and `remove` included.
    ///
    /// Each line with such a call is reported, so a name called twice is
    /// reported twice. The extra is the script name. The Location is `prerm` at
    /// that line. The init-d check ignores comments and the `if [ -x
    /// .../update-rc.d ]` guard, and so do we. Skipped for the `initscripts` and
    /// `sysvinit` packages, as the whole init-d check is. A symlink `prerm` is
    /// read through its target in the same directory, as lintian reads it.
    PrermCallsUpdatercD,
    code = "LN1100",
    tag = "prerm-calls-updaterc.d",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PrermCallsUpdatercD {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if matches!(ctx.package_name(), "initscripts" | "sysvinit") {
            return;
        }
        let Some(control) = ctx.control_files() else {
            return;
        };
        let calls = calls(control);
        for (line, name) in calls {
            ctx.diagnostic(Diagnostic::error(name).with_location(Location {
                path: "prerm".into(),
                line: Some(line),
                column: None,
            }));
        }
    }
}

static EXCLUDE_UPDATE_RC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"if\s+\[\s+-x\s+\S*update-rc\.d").expect("valid regex"));
/// `prerm` form: unanchored, and the action is not needed.
static PRERM_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"update-rc\.d\s+(?:-\S+\s*)*([\w.+][\w\-.+]*)").expect("valid regex")
});

/// `(line, name)` for each `update-rc.d` call in `prerm`.
fn calls(control: &FileTree) -> Vec<(u32, String)> {
    code_lines(control, "prerm")
        .into_iter()
        .filter_map(|(line, code)| Some((line, PRERM_CALL.captures(&code)?[1].to_string())))
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

    fn case(prerm: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(String, TestFile)>::new())
            .control_file("prerm", TestFile::new(prerm))
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            case("# update-rc.d foo remove\n"),
            case("if [ -x \"/usr/sbin/update-rc.d\" ]; then update-rc.d foo remove; fi\n"),
            case("echo hello\n"),
            case("update-rc.d foo remove\n").package("initscripts"),
            case("update-rc.d foo remove\n").package("sysvinit"),
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new())
                .control_file("prerm", TestFile::symlink("prerm.real")),
        ];
        let fail = vec![
            case("update-rc.d foo remove\n"),
            case("update-rc.d -f foo remove\n"),
            case("update-rc.d foo remove\nupdate-rc.d foo remove\n"),
            case("update-rc.d foo remove # keep\nupdate-rc.d bar\n"),
            case("update-rc.d foo remove\n").udeb(),
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new())
                .control_file("prerm", TestFile::symlink("prerm.real"))
                .control_file("prerm.real", TestFile::new("update-rc.d foo remove\n")),
        ];
        BinaryPackageTester::new(PrermCallsUpdatercD, pass, fail).test_and_snapshot();
    }
}
