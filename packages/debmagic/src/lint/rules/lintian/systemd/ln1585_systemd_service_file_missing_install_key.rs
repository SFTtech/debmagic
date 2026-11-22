use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::{BinaryPackageRule, Severity};
use crate::lint::rules::lintian::init_scripts::systemd_unit;

pub struct SystemdServiceFileMissingInstallKey;

declare_rule! {
    /// A standalone service unit has no `[Install]` key that enables it.
    ///
    /// Reported as an info hint with no Extra; the unit path is the Location.
    /// The unit must be a regular file or hardlink at `lib/systemd/<dir>/<name>.service`
    /// (optionally under `usr/`), and not a template (`@.service`). It is
    /// skipped when it has `WantedBy=`, when its `[Service]` `Type` is
    /// `oneshot`, or when a `<name>.path` or `<name>.timer` unit exists beside
    /// it in `lib/systemd/system/` (only that folder counts for the sibling
    /// check). The hint is given unless `[Install]` has `RequiredBy=` or
    /// `Also=`. Values use lintian's assignment rules, so an empty assignment
    /// clears the list.
    SystemdServiceFileMissingInstallKey,
    code = "LN1585",
    tag = "systemd-service-file-missing-install-key",
    default_selected = false,
    experimental = false,
}

static UNIT_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:usr/)?lib/systemd/[^/]+/[^/]+\.service$").expect("valid regex")
});

static SYSTEM_UNIT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(usr/)?lib/systemd/system/([^/]*?)@?\.service$").expect("valid regex")
});

impl BinaryPackageRule for SystemdServiceFileMissingInstallKey {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for path in missing_install_key(files) {
            ctx.diagnostic(
                Diagnostic::warn(String::new())
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

/// Unit paths with no enabling `[Install]` key, in tree order.
fn missing_install_key(files: &FileTree) -> Vec<PathBuf> {
    let mut missing = Vec::new();
    for entry in files.entries() {
        let name = entry.path().to_string_lossy().into_owned();
        if !UNIT_PATH.is_match(&name) || name.ends_with("@.service") {
            continue;
        }
        if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
            continue;
        }
        let Ok(bytes) = files.read(entry) else {
            continue;
        };
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        let lines = systemd_unit::service_lines(&text);
        let wanted_by = systemd_unit::service_values(&lines, "Install", "WantedBy");
        let oneshot = systemd_unit::service_values(&lines, "Service", "Type")
            .iter()
            .any(|kind| kind == "oneshot");
        if !wanted_by.is_empty() || oneshot || !is_standalone(files, &name) {
            continue;
        }
        let enabled = !systemd_unit::service_values(&lines, "Install", "RequiredBy").is_empty()
            || !systemd_unit::service_values(&lines, "Install", "Also").is_empty();
        if !enabled {
            missing.push(entry.path().to_path_buf());
        }
    }
    missing
}

/// lintian's standalone test: no `<name>.path` or `<name>.timer` beside a
/// `lib/systemd/system/` unit, where the name drops a trailing `@`.
fn is_standalone(files: &FileTree, name: &str) -> bool {
    let Some(captures) = SYSTEM_UNIT.captures(name) else {
        return true;
    };
    let usr = captures.get(1).map_or("", |usr| usr.as_str());
    let service = &captures[2];
    let sibling = |kind: &str| {
        let path = format!("{usr}lib/systemd/system/{service}.{kind}");
        files.get(Path::new(&path)).is_some()
    };
    !(sibling("path") || sibling("timer"))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn unit(path: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(body))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            unit(
                "usr/lib/systemd/system/foo.service",
                "[Install]\nWantedBy=multi-user.target\n",
            ),
            unit(
                "usr/lib/systemd/system/foo.service",
                "[Install]\nRequiredBy=bar.target\n",
            ),
            unit(
                "usr/lib/systemd/system/foo.service",
                "[Install]\nAlso=foo.socket\n",
            ),
            unit(
                "usr/lib/systemd/system/foo.service",
                "[Service]\nType=oneshot\n",
            ),
            // A template is not checked.
            unit(
                "usr/lib/systemd/system/foo@.service",
                "[Service]\nExecStart=/bin/true\n",
            ),
            // A .timer beside it makes the unit not standalone.
            BinaryPackageCase::from(vec![
                (
                    "usr/lib/systemd/system/foo.service",
                    TestFile::new("[Service]\nExecStart=/bin/true\n"),
                ),
                (
                    "usr/lib/systemd/system/foo.timer",
                    TestFile::new("[Timer]\n"),
                ),
            ]),
            // A subfolder deeper than the system folder is not matched.
            unit(
                "usr/lib/systemd/system/sub/foo.service",
                "[Service]\nExecStart=/bin/true\n",
            ),
            // A masked unit is not a regular file.
            BinaryPackageCase::from(vec![(
                "usr/lib/systemd/system/foo.service",
                TestFile::symlink("/dev/null"),
            )]),
        ];
        let fail = vec![
            unit(
                "usr/lib/systemd/system/foo.service",
                "[Service]\nExecStart=/bin/true\n",
            ),
            unit("lib/systemd/system/foo.service", "[Unit]\nDescription=x\n"),
            // An empty assignment clears a WantedBy line before it.
            unit(
                "usr/lib/systemd/system/foo.service",
                "[Install]\nWantedBy=multi-user.target\nWantedBy=\n",
            ),
            unit(
                "usr/lib/systemd/system/foo.service",
                "[Service]\nExecStart=/bin/true\n",
            )
            .udeb(),
        ];
        BinaryPackageTester::new(SystemdServiceFileMissingInstallKey, pass, fail)
            .test_and_snapshot();
    }
}
