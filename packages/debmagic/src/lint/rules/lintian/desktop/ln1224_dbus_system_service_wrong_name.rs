use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

pub struct DbusSystemServiceWrongName;

declare_rule! {
    /// A system D-Bus service file's `Name=` does not match its file name.
    ///
    /// Scans the regular files directly inside `usr/share/dbus-1/system-services`
    /// (as text). For each `Name=` line, the file should be named
    /// `<name>.service`; otherwise the Rule reports `better: <name>.service`.
    /// The extra is that text. Location is the path, with no line number.
    /// Symlinks, empty files and non-UTF-8 files are skipped.
    DbusSystemServiceWrongName,
    code = "LN1224",
    tag = "dbus-system-service-wrong-name",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DbusSystemServiceWrongName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let hits = ctx.data_files().map(inspect).unwrap_or_default();
        for (path, extra) in hits {
            ctx.diagnostic(Diagnostic::error(extra).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn inspect(files: &FileTree) -> Vec<(PathBuf, String)> {
    static NAME: OnceLock<Regex> = OnceLock::new();
    let name = NAME.get_or_init(|| Regex::new(r"(?m)^Name=(.*)$").expect("valid name regex"));

    let mut hits = Vec::new();
    for entry in files.entries() {
        let path = entry.path();
        if path.parent() != Some(Path::new(SERVICES)) {
            continue;
        }
        if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
            continue;
        }
        let Ok(bytes) = files.read(entry) else {
            continue;
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        let basename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        for caps in name.captures_iter(text) {
            let service = &caps[1];
            if basename == format!("{service}.service") {
                continue;
            }
            hits.push((path.to_path_buf(), format!("better: {service}.service")));
        }
    }
    hits
}

const SERVICES: &str = "usr/share/dbus-1/system-services";

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(entries: &[(&str, TestFile)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            entries
                .iter()
                .map(|(path, file)| (path.to_string(), file.clone()))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn binary() {
        let pass = vec![
            tree(&[]),
            // The file is named after its service.
            tree(&[(
                "usr/share/dbus-1/system-services/com.example.Foo.service",
                TestFile::new("[D-BUS Service]\nName=com.example.Foo\nExec=/usr/bin/foo\n"),
            )]),
            // Not a system service folder.
            tree(&[(
                "usr/share/dbus-1/services/foo.service",
                TestFile::new("[D-BUS Service]\nName=com.example.Other\n"),
            )]),
            // No Name line.
            tree(&[(
                "usr/share/dbus-1/system-services/foo.service",
                TestFile::new("[D-BUS Service]\nExec=/usr/bin/foo\n"),
            )]),
            // Empty file.
            tree(&[(
                "usr/share/dbus-1/system-services/foo.service",
                TestFile::new(""),
            )]),
            // Symlink.
            tree(&[(
                "usr/share/dbus-1/system-services/foo.service",
                TestFile::symlink("/usr/share/dbus-1/services/foo.service"),
            )]),
        ];
        let fail = vec![
            tree(&[(
                "usr/share/dbus-1/system-services/foo.service",
                TestFile::new("[D-BUS Service]\nName=com.example.Foo\n"),
            )]),
            // Two Name lines, both wrong.
            tree(&[(
                "usr/share/dbus-1/system-services/foo.service",
                TestFile::new("Name=com.example.Foo\nName=com.example.Bar\n"),
            )]),
        ];
        BinaryPackageTester::new(DbusSystemServiceWrongName, pass, fail).test_and_snapshot();
    }
}
