use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::{BinaryPackageRule, Severity};
use crate::lint::rules::lintian::init_scripts::systemd_unit;

pub struct SystemdServiceFileMissingDocumentationKey;

declare_rule! {
    /// A service unit has no `Documentation=` in its `[Unit]` section.
    ///
    /// Units are the regular files and hardlinks named `*.service` under
    /// `systemd/system/`. A masked unit (a symbolic link to `/dev/null`) is
    /// not reported: the oracle has no hint for it, though lintian's code
    /// reaches the check. A value in another section does not count. An
    /// empty assignment is ignored, as lintian's. Extra is empty. The unit
    /// path is the Location. Severity is info.
    SystemdServiceFileMissingDocumentationKey,
    code = "LN1432",
    tag = "systemd-service-file-missing-documentation-key",
    default_selected = false,
    experimental = false,
}

static SERVICE_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/systemd/system/.*\.service$").expect("valid regex"));

impl BinaryPackageRule for SystemdServiceFileMissingDocumentationKey {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for path in missing_documentation(files) {
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

/// Unit paths that lack `[Unit] Documentation=`, in tree order.
fn missing_documentation(files: &FileTree) -> Vec<PathBuf> {
    let mut missing = Vec::new();
    for entry in files.entries() {
        if !SERVICE_PATH.is_match(&entry.path().to_string_lossy()) {
            continue;
        }
        if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
            continue;
        }
        let Some(text) = service_text(files, entry) else {
            continue;
        };
        let lines = systemd_unit::service_lines(&text);
        if systemd_unit::service_values(&lines, "Unit", "Documentation").is_empty() {
            missing.push(entry.path().to_path_buf());
        }
    }
    missing
}

fn service_text(files: &FileTree, entry: &crate::lint::file_tree::Entry) -> Option<String> {
    let bytes = files.read(entry).ok()?;
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn unit(path: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(body))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            unit(
                "usr/lib/systemd/system/a.service",
                "[Unit]\nDocumentation=man:a(8)\n\n[Service]\nExecStart=/bin/true\n",
            ),
            unit(
                "usr/lib/systemd/system/b.service",
                "[Unit]\nDocumentation=https://example.org\nDocumentation=man:b(8)\n",
            ),
            unit("usr/lib/systemd/system/c.timer", "[Timer]\n"),
            unit("usr/share/doc/d.service", "[Service]\n"),
            BinaryPackageCase::from(vec![(
                "lib/systemd/system/masked.service",
                TestFile::symlink("/dev/null"),
            )]),
            unit(
                "usr/lib/systemd/system/h.service",
                "[Unit]\nDocumentation=man:h(8)\n",
            )
            .udeb(),
        ];
        let fail = vec![
            unit(
                "usr/lib/systemd/system/e.service",
                "[Service]\nDocumentation=man:e(8)\nExecStart=/bin/true\n",
            ),
            unit(
                "usr/lib/systemd/system/f.service",
                "[Unit]\nDescription=f\n",
            ),
            unit("etc/systemd/system/g.service", "[Unit]\nDocumentation=\n"),
        ];
        BinaryPackageTester::new(SystemdServiceFileMissingDocumentationKey, pass, fail)
            .test_and_snapshot();
    }
}
