use std::collections::HashMap;
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

use super::manual::{command_from_manual_name, section_page};

pub struct NoEnglishManualPage;

declare_rule! {
    /// A command has a manual page, but none of them is English.
    ///
    /// Compares names in this package's Data files. A command is a file,
    /// hardlink, or symlink whose parent is `bin`, `usr/bin`, `usr/bin/X11`,
    /// `usr/bin/mh`, `usr/games`, `sbin`, `usr/sbin`, or `usr/libexec`. A path
    /// containing `/libexec/` is skipped. A page under `usr/share/man/.../manN/`
    /// documents the command parsed from its filename. English is a page with
    /// no locale directory; `usr/share/man/man/manN/` is locale `man`, not
    /// English. Any English page is enough. The same name in an admin
    /// directory replaces a user directory; otherwise the later sorted path
    /// is kept. Pages shipped by another package are not visible here. Udebs
    /// are skipped. Extra is empty; the Location is the command.
    NoEnglishManualPage,
    code = "LN0277",
    tag = "no-english-manual-page",
    default_selected = true,
    experimental = false,
}

const USER_DIRS: &[&str] = &["bin", "usr/bin", "usr/bin/X11", "usr/bin/mh", "usr/games"];
const ADMIN_DIRS: &[&str] = &["sbin", "usr/sbin", "usr/libexec"];

impl BinaryPackageRule for NoEnglishManualPage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        for path in commands_without_english(files) {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn commands_without_english(files: &FileTree) -> Vec<PathBuf> {
    let documented = documented_languages(files);
    let mut user: HashMap<String, PathBuf> = HashMap::new();
    let mut admin: HashMap<String, PathBuf> = HashMap::new();
    for entry in files.entries() {
        let Some((name, path)) = command(entry) else {
            continue;
        };
        let Some(parent) = entry.path().parent().and_then(|parent| parent.to_str()) else {
            continue;
        };
        if USER_DIRS.contains(&parent) {
            user.insert(name, path);
        } else if ADMIN_DIRS.contains(&parent) {
            admin.insert(name, path);
        }
    }
    for (name, path) in admin {
        user.insert(name, path);
    }
    let mut missing: Vec<PathBuf> = user
        .into_iter()
        .filter(|(name, path)| {
            documented.get(name).is_some_and(|english| !english)
                && !path.to_str().is_some_and(|path| path.contains("/libexec/"))
        })
        .map(|(_, path)| path)
        .collect();
    missing.sort();
    missing
}

/// `true` when any page for that command has no locale directory.
fn documented_languages(files: &FileTree) -> HashMap<String, bool> {
    let mut documented = HashMap::new();
    for entry in files.entries() {
        let Some(page) = section_page(entry) else {
            continue;
        };
        let Some(command) = command_from_manual_name(page.file_name) else {
            continue;
        };
        let english = page.language.is_none();
        documented
            .entry(command)
            .and_modify(|flag| *flag |= english)
            .or_insert(english);
    }
    documented
}

fn command(entry: &Entry) -> Option<(String, PathBuf)> {
    if !matches!(
        entry.kind(),
        EntryKind::RegularFile | EntryKind::Hardlink | EntryKind::Symlink
    ) {
        return None;
    }
    let path = entry.path().to_str()?;
    let name = entry.path().file_name()?.to_str()?.to_string();
    Some((name, PathBuf::from(path)))
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
    fn test() {
        let pass = vec![
            case(vec![]),
            case(vec![
                text("usr/bin/foo"),
                text("usr/share/man/man1/foo.1.gz"),
            ]),
            case(vec![
                text("usr/bin/foo"),
                text("usr/share/man/man1/foo.1.gz"),
                text("usr/share/man/de/man1/foo.1.gz"),
            ]),
            // No page at all is `no-manual-page`, not this Tag.
            case(vec![text("usr/bin/foo")]),
            case(vec![text("usr/share/man/de/man1/orphan.1.gz")]),
            case(vec![
                text("usr/libexec/telekinetic"),
                text("usr/share/man/de/man1/telekinetic.1.gz"),
            ]),
            case(vec![("usr/bin/foo".to_string(), TestFile::symlink("bar"))]),
            // An English symlink page is enough.
            case(vec![
                text("usr/bin/foo"),
                (
                    "usr/share/man/man1/foo.1.gz".to_string(),
                    TestFile::symlink("undocumented.7.gz"),
                ),
            ]),
            case(vec![
                text("usr/bin/foo"),
                text("usr/share/man/de/man1/foo.1.gz"),
            ])
            .udeb(),
        ];
        let fail = vec![
            case(vec![
                text("usr/bin/alpha"),
                text("usr/share/man/de/man1/alpha.1.gz"),
                text("usr/bin/beta"),
                text("usr/share/man/fr/man8/beta.8.gz"),
                text("usr/games/game"),
                text("usr/share/man/ja/man6/game.6.gz"),
            ]),
            // Locale directory `man` is not English.
            case(vec![
                text("usr/bin/foo"),
                text("usr/share/man/man/man1/foo.1.gz"),
            ]),
            case(vec![
                text("usr/bin/from-abroad"),
                (
                    "usr/share/man/de/man1/from-abroad.1.gz".to_string(),
                    TestFile::symlink("other.1.gz"),
                ),
            ]),
            // An admin directory replaces a user directory.
            case(vec![
                text("usr/bin/helper"),
                text("sbin/helper"),
                text("usr/share/man/de/man8/helper.8.gz"),
            ]),
            // The later sorted user path wins.
            case(vec![
                text("bin/tool"),
                text("usr/bin/tool"),
                text("usr/share/man/de/man1/tool.1.gz"),
            ]),
            case(vec![
                ("usr/bin/foo".to_string(), TestFile::new("elf").hardlink()),
                text("usr/share/man/de/man1/foo.1.gz"),
            ]),
        ];
        BinaryPackageTester::new(NoEnglishManualPage, pass, fail).test_and_snapshot();
    }
}
