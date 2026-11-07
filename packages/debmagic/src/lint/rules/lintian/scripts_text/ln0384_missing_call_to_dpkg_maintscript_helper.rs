use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MissingCallToDpkgMaintscriptHelper;

declare_rule! {
    /// `dpkg-maintscript-helper` is missing from one of `preinst`, `postinst`,
    /// or `postrm`.
    ///
    /// When any maintainer script with an interpreter calls a helper command,
    /// each of those three scripts must call the same command. `prerm` is not
    /// required. Extra is the command, and `[script]` when that script is
    /// absent. The Location is the existing script. Udebs are not checked.
    MissingCallToDpkgMaintscriptHelper,
    code = "LN0384",
    tag = "missing-call-to-dpkg-maintscript-helper",
    default_selected = true,
    experimental = false,
}

const WANTED: &[&str] = &["preinst", "postinst", "postrm"];

static HELPER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        "{}(?:/usr/bin/)?dpkg-maintscript-helper\\s(\\S+)",
        scan::LEADING_PATTERN
    ))
    .expect("valid regex")
});

impl BinaryPackageRule for MissingCallToDpkgMaintscriptHelper {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.control_files() else {
            return;
        };
        let mut seen: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for line in scan::script_lines(files) {
            let Some(command) = HELPER
                .captures(&line.text)
                .and_then(|captures| captures.get(1))
                .map(|m| m.as_str().to_string())
            else {
                continue;
            };
            let Some(name) = line.path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            seen.entry(command).or_default().insert(name.to_string());
        }
        let mut missing = Vec::new();
        for (command, have) in seen {
            for name in WANTED {
                if have.contains(*name) {
                    continue;
                }
                missing.push((command.clone(), *name, script_exists(files, name)));
            }
        }
        for (command, name, exists) in missing {
            if exists {
                ctx.diagnostic(Diagnostic::error(command).with_location(Location {
                    path: Path::new(name).to_path_buf(),
                    line: None,
                    column: None,
                }));
            } else {
                ctx.diagnostic(Diagnostic::error(format!("{command} [{name}]")));
            }
        }
    }
}

fn script_exists(files: &FileTree, name: &str) -> bool {
    files
        .get(name)
        .is_some_and(|entry| matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn scripts(files: &[(&str, &str)]) -> BinaryPackageCase {
        let mut case = BinaryPackageCase::from(Vec::<(&str, TestFile)>::new());
        for (name, content) in files {
            case = case.control_file(*name, TestFile::new(*content));
        }
        case
    }

    #[test]
    fn test() {
        let call = "#!/bin/sh\ndpkg-maintscript-helper symlink_to_dir /a /b pkg -- \"$@\"\n";
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            scripts(&[("postinst", "#!/bin/sh\ntrue\n")]),
            scripts(&[("preinst", call), ("postinst", call), ("postrm", call)]),
            scripts(&[("postinst", call)]).udeb(),
            scripts(&[("postinst", "dpkg-maintscript-helper symlink_to_dir\n")]),
        ];
        let fail = vec![
            scripts(&[("postinst", call), ("preinst", call)]),
            scripts(&[("postinst", call)]),
            scripts(&[("prerm", call)]),
        ];
        BinaryPackageTester::new(MissingCallToDpkgMaintscriptHelper, pass, fail)
            .test_and_snapshot();
    }
}
