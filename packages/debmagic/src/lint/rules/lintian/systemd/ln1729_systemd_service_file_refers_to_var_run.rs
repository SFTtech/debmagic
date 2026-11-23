use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::{BinaryPackageRule, Severity};
use crate::lint::rules::lintian::init_scripts::systemd_unit;

pub struct SystemdServiceFileRefersToVarRun;

declare_rule! {
    /// A systemd unit names a path under `/var/run/`.
    ///
    /// Two keys are read. In a `[Service]` section, `PIDFile=` values that
    /// start with `/var/run/` are reported, and in a `[Socket]` section of a
    /// `*.socket` unit under `systemd/system/`, `ListenStream=` values that
    /// start with `/var/run/` are reported. Extra is `PIDFile <value>` or
    /// `ListenStream <value>`; the Location is the unit. Only regular files and
    /// hardlinks are read, so a symlinked unit is not checked (lintian would
    /// follow a link into the tree). Continuation lines, comments, and empty
    /// assignments follow the shared unit reader. Severity is info and the Tag
    /// is not selected by default, as in lintian. Udebs are checked too.
    SystemdServiceFileRefersToVarRun,
    code = "LN1729",
    tag = "systemd-service-file-refers-to-var-run",
    default_selected = false,
    experimental = false,
}

static SOCKET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/systemd/system/.*\.socket$").expect("valid regex"));

impl BinaryPackageRule for SystemdServiceFileRefersToVarRun {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits: Vec<(PathBuf, String)> = Vec::new();
        for (path, text) in systemd_unit::service_texts(files) {
            let lines = systemd_unit::service_lines(&text);
            for value in systemd_unit::service_values(&lines, "Service", "PIDFile") {
                if value.starts_with("/var/run/") {
                    hits.push((path.clone(), format!("PIDFile {value}")));
                }
            }
        }
        for (path, text) in unit_texts(files, &SOCKET) {
            let lines = systemd_unit::service_lines(&text);
            for value in systemd_unit::service_values(&lines, "Socket", "ListenStream") {
                if value.starts_with("/var/run/") {
                    hits.push((path.clone(), format!("ListenStream {value}")));
                }
            }
        }
        for (path, extra) in hits {
            ctx.diagnostic(
                Diagnostic::warn(extra)
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

/// Text of the regular files and hardlinks whose path matches `pattern`.
fn unit_texts(files: &FileTree, pattern: &Regex) -> Vec<(PathBuf, String)> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .path(pattern)
        .into_iter()
        .filter_map(|entry| {
            let bytes = files.read(entry).ok()?;
            Some((
                entry.path().to_path_buf(),
                String::from_utf8_lossy(&bytes).into_owned(),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn unit(path: &str, text: &str) -> (String, TestFile) {
        (path.to_string(), TestFile::new(text))
    }

    fn case(entries: Vec<(String, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(entries)
    }

    #[test]
    fn binary() {
        let pass = vec![
            case(vec![]),
            // A PIDFile in /run is the modern place.
            case(vec![unit(
                "lib/systemd/system/foo.service",
                "[Service]\nPIDFile=/run/foo.pid\n",
            )]),
            // A PIDFile key outside [Service] is ignored.
            case(vec![unit(
                "lib/systemd/system/foo.service",
                "[Unit]\nPIDFile=/var/run/foo.pid\n",
            )]),
            // A commented or empty-reset assignment.
            case(vec![unit(
                "lib/systemd/system/foo.service",
                "[Service]\n#PIDFile=/var/run/foo.pid\n",
            )]),
            case(vec![unit(
                "lib/systemd/system/foo.service",
                "[Service]\nPIDFile=/var/run/foo.pid\nPIDFile=\n",
            )]),
            // Not under /var/run (a prefix lookalike is not /var/run/).
            case(vec![unit(
                "lib/systemd/system/foo.service",
                "[Service]\nPIDFile=/var/runner/foo.pid\n",
            )]),
            // A ListenStream in /run, and one outside systemd/system.
            case(vec![unit(
                "lib/systemd/system/foo.socket",
                "[Socket]\nListenStream=/run/foo.sock\n",
            )]),
            case(vec![unit(
                "etc/foo/foo.socket",
                "[Socket]\nListenStream=/var/run/foo.sock\n",
            )]),
            // A symlinked unit is not read.
            case(vec![(
                "lib/systemd/system/foo.service".to_string(),
                TestFile::symlink("bar.service"),
            )]),
        ];
        let fail = vec![
            case(vec![unit(
                "lib/systemd/system/foo.service",
                "[Service]\nPIDFile=/var/run/foo.pid\n",
            )]),
            // A continuation line and a quoted value.
            case(vec![unit(
                "lib/systemd/system/foo.service",
                "[Service]\nPIDFile=\"/var/run/\\\nfoo.pid\"\n",
            )]),
            // A socket unit under systemd/system.
            case(vec![unit(
                "usr/lib/systemd/system/foo.socket",
                "[Socket]\nListenStream=/var/run/foo.sock\n",
            )]),
            // Udebs are checked too.
            case(vec![unit(
                "lib/systemd/system/foo.service",
                "[Service]\nPIDFile=/var/run/foo.pid\n",
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(SystemdServiceFileRefersToVarRun, pass, fail).test_and_snapshot();
    }
}
