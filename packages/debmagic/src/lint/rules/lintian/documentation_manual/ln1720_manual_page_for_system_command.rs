use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, FileTree};
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::manual::{command_from_manual_name, section_page};

pub struct ManualPageForSystemCommand;

declare_rule! {
    /// A system administration command has a section 1 manual page.
    ///
    /// A command is a file or symlink directly in `sbin`, `usr/sbin`, or
    /// `usr/libexec`; when a name repeats, the later sorted path is kept. The
    /// Tag fires on that entry when it is a regular file (not a symlink or
    /// hardlink) and a manual page in this package documents the command in
    /// a `usr/share/man/.../man1/` directory. The directory section is the
    /// one that counts, not the file name's section. Manual pages from other
    /// packages are not visible here, so a dependency's page that shadows the
    /// local one is not modelled. Udebs are skipped. Extra is empty; the
    /// Location is the command.
    ManualPageForSystemCommand,
    code = "LN1720",
    tag = "manual-page-for-system-command",
    default_selected = false,
    experimental = false,
}

const ADMIN_DIRS: &[&str] = &["sbin", "usr/sbin", "usr/libexec"];
const USER_COMMAND_SECTION: char = '1';

impl BinaryPackageRule for ManualPageForSystemCommand {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        for path in admin_commands_with_section_one_page(files) {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

/// Paths of regular-file admin commands whose name has a section 1 page.
fn admin_commands_with_section_one_page(files: &FileTree) -> Vec<PathBuf> {
    let section_one = section_one_commands(files);

    // Later sorted path wins, as lintian's hash assignment does.
    let mut admin: HashMap<String, &Entry> = HashMap::new();
    for entry in files.entries() {
        if !(entry.is_file() || entry.is_symlink()) {
            continue;
        }
        let Some(parent) = entry.path().parent().and_then(|parent| parent.to_str()) else {
            continue;
        };
        if !ADMIN_DIRS.contains(&parent) {
            continue;
        }
        let Some(name) = entry.path().file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        admin.insert(name.to_string(), entry);
    }

    let mut found: Vec<PathBuf> = admin
        .into_iter()
        .filter(|(name, entry)| entry.is_regular_file() && section_one.contains(name))
        .map(|(_, entry)| entry.path().to_path_buf())
        .collect();
    found.sort();
    found
}

/// Commands documented by a page in a `usr/share/man/.../man1/` directory.
fn section_one_commands(files: &FileTree) -> HashSet<String> {
    files
        .entries()
        .iter()
        .filter_map(|entry| {
            let page = section_page(entry)?;
            let directory_section = page.page_path.strip_suffix('/')?.chars().last()?;
            if directory_section != USER_COMMAND_SECTION {
                return None;
            }
            command_from_manual_name(page.file_name)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn text(path: &str) -> (String, TestFile) {
        (path.to_string(), TestFile::new("x"))
    }

    fn case(entries: Vec<(String, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(entries)
    }

    #[test]
    fn binary() {
        let pass = vec![
            case(vec![]),
            // A user command with a section 1 page is fine.
            case(vec![
                text("usr/bin/foo"),
                text("usr/share/man/man1/foo.1.gz"),
            ]),
            // Section 8 is the right place for an admin command's page.
            case(vec![
                text("usr/sbin/sbin-binary"),
                text("usr/share/man/man8/sbin-binary.8.gz"),
            ]),
            // The file name section does not count, only the directory.
            case(vec![
                text("usr/sbin/foo"),
                text("usr/share/man/man8/foo.1.gz"),
            ]),
            // A symlink admin command is not a regular file.
            case(vec![
                ("usr/sbin/foo".to_string(), TestFile::symlink("bar")),
                text("usr/share/man/man1/foo.1.gz"),
            ]),
            // A hardlink admin command is not a regular file either.
            case(vec![
                text("usr/sbin/foo"),
                (
                    "usr/sbin/bar".to_string(),
                    TestFile::new("x").hardlink_to("usr/sbin/foo"),
                ),
                text("usr/share/man/man1/bar.1.gz"),
            ]),
            // A symlink page documents the command, but a section 8 page here
            // is still the right place.
            case(vec![
                text("usr/sbin/foo"),
                (
                    "usr/share/man/man8/foo.8.gz".to_string(),
                    TestFile::symlink("undocumented.8.gz"),
                ),
            ]),
            // Locale directory names do not stop the section check.
            case(vec![
                text("usr/sbin/foo"),
                text("usr/share/man/de/man8/foo.8.gz"),
            ]),
            // Only sbin, usr/sbin, and usr/libexec are admin locations.
            case(vec![
                text("usr/bin/foo"),
                text("usr/share/man/man1/foo.1.gz"),
            ]),
            case(vec![
                text("usr/libexec/foo/bar"),
                text("usr/share/man/man1/bar.1.gz"),
            ]),
            // A page for a different command name.
            case(vec![
                text("usr/sbin/foo"),
                text("usr/share/man/man1/bar.1.gz"),
            ]),
            // No page in section 1 under a section 1 name without a directory.
            case(vec![
                text("usr/sbin/foo"),
                text("usr/share/man/orphan.1.gz"),
            ]),
            case(vec![text("usr/sbin/foo")]).udeb(),
        ];
        let fail = vec![
            // The page for the admin command is in man1.
            case(vec![
                text("usr/sbin/usr-sbin-binary"),
                text("usr/share/man/man1/usr-sbin-binary.1.gz"),
            ]),
            // Plain sbin and usr/libexec too.
            case(vec![text("sbin/foo"), text("usr/share/man/man1/foo.1.gz")]),
            case(vec![
                text("usr/libexec/foo"),
                text("usr/share/man/man1/foo.1.gz"),
            ]),
            // A locale page in man1 counts.
            case(vec![
                text("usr/sbin/foo"),
                text("usr/share/man/de/man1/foo.1.gz"),
            ]),
            // Both: the name repeats in sorted order; the later path wins and is a file.
            case(vec![
                ("sbin/foo".to_string(), TestFile::symlink("bar")),
                text("usr/sbin/foo"),
                text("usr/share/man/man1/foo.1.gz"),
            ]),
        ];
        BinaryPackageTester::new(ManualPageForSystemCommand, pass, fail).test_and_snapshot();
    }
}
