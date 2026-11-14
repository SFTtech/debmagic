use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

use super::manual::{command_from_manual_name, section_page};

pub struct NoManualPage;

declare_rule! {
    /// A command in this package has no manual page.
    ///
    /// Compares names in this package's Data files. A command is a file,
    /// hardlink, or symlink whose parent is `bin`, `usr/bin`, `usr/bin/X11`,
    /// `usr/bin/mh`, `usr/games`, `sbin`, `usr/sbin`, or `usr/libexec`. A path
    /// containing `/libexec/` is skipped. A page under `usr/share/man/.../manN/`
    /// documents the command parsed from its filename (a trailing `gz`
    /// component is dropped, the section must start with a digit, and the rest
    /// is joined by `.`). Any locale counts, including a symlink page. The same
    /// name in an admin directory replaces a user directory; otherwise the
    /// later sorted path is kept. Pages shipped by another package are not
    /// visible here. Udebs are skipped. Extra is empty; the Location is the
    /// command.
    NoManualPage,
    code = "LN0152",
    tag = "no-manual-page",
    default_selected = true,
    experimental = false,
}

const USER_DIRS: &[&str] = &["bin", "usr/bin", "usr/bin/X11", "usr/bin/mh", "usr/games"];
const ADMIN_DIRS: &[&str] = &["sbin", "usr/sbin", "usr/libexec"];

impl BinaryPackageRule for NoManualPage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        for path in commands_without_manual(files) {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn commands_without_manual(files: &FileTree) -> Vec<PathBuf> {
    let documented = documented_commands(files);
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
            !documented.contains(name)
                && !path.to_str().is_some_and(|path| path.contains("/libexec/"))
        })
        .map(|(_, path)| path)
        .collect();
    missing.sort();
    missing
}

fn documented_commands(files: &FileTree) -> HashSet<String> {
    files
        .entries()
        .iter()
        .filter_map(|entry| {
            let page = section_page(entry)?;
            command_from_manual_name(page.file_name)
        })
        .collect()
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
            // A locale page still documents the command.
            case(vec![
                text("usr/bin/binary-without-english-manpage"),
                text("usr/share/man/de/man1/binary-without-english-manpage.1"),
            ]),
            // `1p` is a section suffix. `foo.en.1.gz` documents `foo.en`.
            case(vec![
                text("usr/bin/test"),
                text("usr/share/man/man1/test.1p.gz"),
                text("usr/bin/foo.en"),
                text("usr/share/man/man1/foo.en.1.gz"),
            ]),
            case(vec![
                text("usr/bin/hello.static"),
                text("usr/share/man/man1/hello.static.1.gz"),
            ]),
            // Section directory and filename section need not match.
            case(vec![
                text("usr/sbin/addpart"),
                text("usr/share/man/man8/addpart.1.gz"),
            ]),
            // A symlink page counts, and so does a page whose bytes are not gzip.
            case(vec![
                text("usr/bin/sbin-binary"),
                (
                    "usr/share/man/man1/sbin-binary.1.gz".to_string(),
                    TestFile::symlink("undocumented.7.gz"),
                ),
                text("usr/bin/usr-bin-binary-alt"),
                (
                    "usr/share/man/man1/usr-bin-binary-alt.1.gz".to_string(),
                    TestFile::new("not gzip"),
                ),
            ]),
            // Trailing dot: Perl `split` still sees a `gz` component.
            case(vec![
                text("usr/bin/foo"),
                text("usr/share/man/man1/foo.1.gz."),
            ]),
            case(vec![text("usr/libexec/telekinetic")]),
            case(vec![text("usr/lib/not-a-command")]),
            case(vec![text("usr/share/man/man1/orphan.1.gz")]),
            case(vec![("usr/bin".to_string(), TestFile::directory())]),
            case(vec![text("usr/bin/foo")]).udeb(),
        ];
        let fail = vec![
            case(vec![
                text("bin/pwd"),
                text("sbin/addpart"),
                text("usr/bin/X11/testxbin"),
                text("usr/bin/mh/read"),
                text("usr/bin/telepathic"),
                text("usr/games/yet-another-test-game"),
                text("usr/sbin/helper"),
            ]),
            case(vec![(
                "usr/bin/rstartd".to_string(),
                TestFile::symlink("usr-bin-binary"),
            )]),
            case(vec![(
                "usr/bin/static-hello".to_string(),
                TestFile::new("elf").hardlink(),
            )]),
            // X11 manual directories are not `usr/share/man`.
            case(vec![
                text("usr/bin/rstartd"),
                text("usr/X11R6/man/man1/rstartd.1x.gz"),
            ]),
            // `simple.pod.gz` has no section digit. Case must match.
            case(vec![
                text("usr/bin/simple"),
                text("usr/share/man/man6/simple.pod.gz"),
                text("usr/bin/foo"),
                text("usr/share/man/man1/Foo.1.gz"),
            ]),
            // `foo.en.1.gz` documents `foo.en`, not `foo`.
            case(vec![
                text("usr/bin/foo"),
                text("usr/share/man/man1/foo.en.1.gz"),
            ]),
            // Same user-directory name: the later sorted path wins.
            case(vec![text("bin/tool"), text("usr/bin/tool")]),
            // An admin directory replaces a user directory.
            case(vec![text("sbin/helper"), text("usr/bin/helper")]),
        ];
        BinaryPackageTester::new(NoManualPage, pass, fail).test_and_snapshot();
    }
}
