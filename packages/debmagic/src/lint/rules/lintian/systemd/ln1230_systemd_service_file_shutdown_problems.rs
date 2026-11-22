use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, follow_symlinks};
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;
use crate::lint::rules::lintian::init_scripts::systemd_unit::{service_lines, service_values};

pub struct SystemdServiceFileShutdownProblems;

declare_rule! {
    /// A service unit under a `systemd/system/` directory has a `[Unit]`
    /// `DefaultDependencies=` assignment and a `Conflicts=shutdown.target`, but no
    /// `Before=shutdown.target`. The unit may linger until the end of shutdown.
    ///
    /// Like lintian, any non-empty `DefaultDependencies` value counts, not only
    /// `no`. Units are read through in-tree symlinks; a unit that is a symlink to
    /// `/dev/null` (masked) is skipped. Extra is empty; the unit path is the
    /// Location. Udebs are checked too.
    SystemdServiceFileShutdownProblems,
    code = "LN1230",
    tag = "systemd-service-file-shutdown-problems",
    default_selected = true,
    experimental = false,
}

static SERVICE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/systemd/system/.*\.service$").expect("valid regex"));

impl BinaryPackageRule for SystemdServiceFileShutdownProblems {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut paths = Vec::new();
        for entry in files.entries() {
            // Directories are never units: lintian's names of them end in `/`.
            if entry.is_dir() || !SERVICE.is_match(&entry.path().to_string_lossy()) {
                continue;
            }
            if entry.is_symlink() && entry.link_target() == Some("/dev/null".as_ref()) {
                continue;
            }
            let Some(target) = follow_symlinks(files, entry) else {
                continue;
            };
            let Ok(bytes) = files.read(target) else {
                continue;
            };
            let Ok(text) = String::from_utf8(bytes) else {
                continue;
            };
            let lines = service_lines(&text);
            if service_values(&lines, "Unit", "DefaultDependencies").is_empty() {
                continue;
            }
            let before = service_values(&lines, "Unit", "Before");
            let conflicts = service_values(&lines, "Unit", "Conflicts");
            if !before.iter().any(|unit| unit == "shutdown.target")
                && conflicts.iter().any(|unit| unit == "shutdown.target")
            {
                paths.push(entry.path().to_path_buf());
            }
        }
        for path in paths {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    const UNIT: &str = "usr/lib/systemd/system/foo.service";

    fn unit(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(UNIT, TestFile::new(body))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            unit("[Unit]\nConflicts=shutdown.target\n"),
            unit(
                "[Unit]\nDefaultDependencies=no\nBefore=shutdown.target\nConflicts=shutdown.target\n",
            ),
            unit("[Unit]\nDefaultDependencies=no\n"),
            // An empty assignment clears the earlier DefaultDependencies.
            unit(
                "[Unit]\nDefaultDependencies=no\nDefaultDependencies=\nConflicts=shutdown.target\n",
            ),
            // Only in another section.
            unit("[Service]\nDefaultDependencies=no\nConflicts=shutdown.target\n"),
            // Masked unit.
            BinaryPackageCase::from(vec![(UNIT, TestFile::symlink("/dev/null"))]),
            // Not a unit file.
            BinaryPackageCase::from(vec![(
                "usr/lib/systemd/system/foo.socket",
                TestFile::new("[Unit]\nDefaultDependencies=no\nConflicts=shutdown.target\n"),
            )]),
            // Not under systemd/system.
            BinaryPackageCase::from(vec![(
                "usr/lib/systemd/foo.service",
                TestFile::new("[Unit]\nDefaultDependencies=no\nConflicts=shutdown.target\n"),
            )]),
        ];
        let fail = vec![
            unit("[Unit]\nDefaultDependencies=no\nConflicts=shutdown.target\n"),
            // Lintian's check is for any value of DefaultDependencies.
            unit("[Unit]\nDefaultDependencies=yes\nConflicts=shutdown.target\n"),
            // Read through an in-tree symlink.
            BinaryPackageCase::from(vec![
                (
                    "lib/systemd/system/foo.service",
                    TestFile::symlink("bar.service"),
                ),
                (
                    "lib/systemd/system/bar.service",
                    TestFile::new("[Unit]\nDefaultDependencies=no\nConflicts=shutdown.target\n"),
                ),
            ]),
            // A udeb is still a Binary package.
            unit("[Unit]\nDefaultDependencies=no\nConflicts=shutdown.target\n").udeb(),
        ];
        BinaryPackageTester::new(SystemdServiceFileShutdownProblems, pass, fail)
            .test_and_snapshot();
    }
}
