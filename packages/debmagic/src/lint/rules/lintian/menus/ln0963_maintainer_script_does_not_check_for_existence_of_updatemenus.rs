use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

pub struct MaintainerScriptDoesNotCheckForExistenceOfUpdatemenus;

declare_rule! {
    /// A maintainer script runs `update-menus` without first checking that it
    /// exists.
    ///
    /// Scripts are `preinst`, `postinst`, `prerm` and `postrm`. A control
    /// file that is a symlink, is not a regular file, or is an ELF binary is
    /// skipped. Comments are stripped. A call is `update-menus` or
    /// `/usr/bin/update-menus` at the start of a line, after `;`, `&`, `|`,
    /// `then`, `do` or `exec`. A check is `-x ... update-menus`,
    /// `which update-menus`, `type update-menus` or `command ... update-menus`,
    /// on the same line or an earlier one. Only the first unchecked call in
    /// each script is reported, at its line. The package `menu` is exempt.
    /// Extra is empty. Location is the script with its line.
    MaintainerScriptDoesNotCheckForExistenceOfUpdatemenus,
    code = "LN0963",
    tag = "maintainer-script-does-not-check-for-existence-of-updatemenus",
    default_selected = true,
    experimental = false,
}

/// The maintainer scripts lintian's `menus` Check looks at, in its order.
pub(super) const SCRIPTS: [&str; 4] = ["preinst", "postinst", "prerm", "postrm"];

/// Which command a script calls without checking for it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Command {
    UpdateMenus,
    InstallDocs,
}

static UPDATE_MENUS_CHECK: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"-x\s+\S*update-menus",
        r"(?:which|type)\s+update-menus",
        r"command\s+.*?update-menus",
    ]
    .into_iter()
    .map(|pattern| Regex::new(pattern).expect("valid regex"))
    .collect()
});
static UPDATE_MENUS_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^\s*|[;&|]\s*|(?:then|do|exec)\s+)(?:/usr/bin/)?update-menus(?:\s|[;&|<>]|$)")
        .expect("valid regex")
});
static INSTALL_DOCS_DROP_X: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"-x\s+\S*install-docs").expect("valid regex"));
static INSTALL_DOCS_WHICH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:which|type)\s+install-docs").expect("valid regex"));
static INSTALL_DOCS_DROP_COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"command\s+.*?install-docs").expect("valid regex"));
static INSTALL_DOCS_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^\s*|[;&|]\s*|(?:then|do)\s+)(?:/usr/sbin/)?install-docs(?:\s|[;&|<>]|$)")
        .expect("valid regex")
});

/// The first unchecked call of `command` in each maintainer script, as the
/// script's path and line. `package` is the Binary package name; `menu`
/// never reports an update-menus call.
pub(super) fn unchecked_calls(
    files: &FileTree,
    package: &str,
    command: Command,
) -> Vec<(PathBuf, u32)> {
    SCRIPTS
        .iter()
        .filter_map(|script| {
            let text = script_text(files, script)?;
            first_unchecked(&text, package, command).map(|line| (PathBuf::from(script), line))
        })
        .collect()
}

/// A maintainer script as text, or `None` for a symlink, a non-file and an
/// ELF binary.
fn script_text(files: &FileTree, name: &str) -> Option<String> {
    let entry = files.get(name)?;
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return None;
    }
    let bytes = files.read(entry).ok()?;
    if bytes.starts_with(b"\x7fELF") {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Lintian's line walk for one script. A check, once seen, stays in force
/// for the rest of the script.
fn first_unchecked(text: &str, package: &str, command: Command) -> Option<u32> {
    let mut checked = false;
    for (index, raw) in text.split('\n').enumerate() {
        let line_number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        // Comments are stripped before anything else.
        let line = raw.split('#').next().unwrap_or("");
        match command {
            Command::UpdateMenus => {
                if UPDATE_MENUS_CHECK
                    .iter()
                    .any(|pattern| pattern.is_match(line))
                {
                    checked = true;
                }
                if UPDATE_MENUS_CALL.is_match(line) && !checked && package != "menu" {
                    return Some(line_number);
                }
            }
            Command::InstallDocs => {
                // lintian deletes the existence probe before it looks for the call.
                let mut current = line.to_string();
                if INSTALL_DOCS_DROP_X.is_match(&current) {
                    current = INSTALL_DOCS_DROP_X.replacen(&current, 1, "").into_owned();
                    checked = true;
                } else if INSTALL_DOCS_WHICH.is_match(&current) {
                    checked = true;
                } else if INSTALL_DOCS_DROP_COMMAND.is_match(&current) {
                    current = INSTALL_DOCS_DROP_COMMAND
                        .replacen(&current, 1, "")
                        .into_owned();
                    checked = true;
                }
                if INSTALL_DOCS_CALL.is_match(&current) && !checked {
                    return Some(line_number);
                }
            }
        }
    }
    None
}

impl BinaryPackageRule for MaintainerScriptDoesNotCheckForExistenceOfUpdatemenus {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let calls = {
            let Some(files) = ctx.control_files() else {
                return;
            };
            unchecked_calls(files, &package, Command::UpdateMenus)
        };
        for (path, line) in calls {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn control(name: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(body))
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            control("postinst", "#!/bin/sh\ntrue\n"),
            // Checked on the same line, or on an earlier one.
            control(
                "postinst",
                "#!/bin/sh\nwhich update-menus && update-menus\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\nif [ -x /usr/bin/update-menus ]; then\n  /usr/bin/update-menus\nfi\n",
            ),
            control("postinst", "#!/bin/sh\n# update-menus\n"),
            control("postinst", "#!/bin/sh\nupdate-menus-extra\n"),
            // The menu package itself is exempt.
            control("postinst", "#!/bin/sh\nupdate-menus\n").package("menu"),
            // A symlink is not read.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::symlink("missing")),
            control("postinst", "\x7fELF\nupdate-menus\n"),
        ];
        let fail = vec![
            control("postinst", "#!/bin/sh\nupdate-menus\n"),
            control("postinst", "#!/bin/sh\nset -e\nthen update-menus\n"),
            control("preinst", "#!/bin/sh\nupdate-menus\n"),
            control("postrm", "#!/bin/sh\n/usr/bin/update-menus\n"),
            // A check on a later line is too late.
            control("prerm", "#!/bin/sh\nupdate-menus\nwhich update-menus\n"),
        ];
        BinaryPackageTester::new(
            MaintainerScriptDoesNotCheckForExistenceOfUpdatemenus,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
