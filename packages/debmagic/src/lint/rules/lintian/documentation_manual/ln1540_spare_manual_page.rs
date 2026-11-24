use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::group_context::GroupContext;
use crate::lint::rule::{GroupRule, Severity};

use super::manual::{command_from_manual_name, section_page};

pub struct SpareManualPage;

declare_rule! {
    /// A section 1 or 8 manual page documents no command that this package
    /// has, nor one that a package depending on it ships.
    ///
    /// A command is a file, hardlink, or symlink in `bin`, `usr/bin`,
    /// `usr/bin/X11`, `usr/bin/mh`, `usr/games`, `sbin`, `usr/sbin` or
    /// `usr/libexec`, by its name without a suffix (`foo.sh` is `foo`) for a
    /// reliant; for this package the name as is. The commands are those of
    /// this installable and of the installables of the Group that strongly
    /// depend on it (`GroupContext::direct_reliants`). A manual page under
    /// `usr/share/man/.../man1` or `man8` whose command (from its filename,
    /// as `no-manual-page` reads it) is none of those, and does not start with
    /// one of them followed by a word boundary or `_`, is reported. Other
    /// sections are not. The Location is the page. Udebs are skipped.
    ///
    /// A Group Rule: it reports on the installable that has the page, and runs
    /// once per Group, with or without a `.dsc` (lintian's per-binary check
    /// reads no source, so a lone `.deb` is checked as well; ADR 0056).
    /// Differences from lintian: the reliant
    /// commands are read from the reliant's Data files, which lintian reads
    /// from its installed list; for both, a sibling whose files cannot be read
    /// adds nothing.
    SpareManualPage,
    code = "LN1540",
    tag = "spare-manual-page",
    default_selected = false,
    experimental = false,
}

const USER_DIRS: &[&str] = &["bin", "usr/bin", "usr/bin/X11", "usr/bin/mh", "usr/games"];
const ADMIN_DIRS: &[&str] = &["sbin", "usr/sbin", "usr/libexec"];

impl GroupRule for SpareManualPage {
    fn runs_without_dsc(&self) -> bool {
        true
    }

    fn run(&self, ctx: &mut GroupContext<'_>) {
        let installables = ctx.installables();
        for installable in &installables {
            if installable.is_udeb() {
                continue;
            }
            let Some(files) = ctx.data_files(installable) else {
                continue;
            };
            let mut related = commands_in(files, false);
            for reliant in ctx.direct_reliants(installable) {
                let Some(reliant_files) = ctx.data_files(&reliant) else {
                    continue;
                };
                related.extend(commands_in(reliant_files, true));
            }
            let mut findings: Vec<PathBuf> = surplus_pages(files, &related);
            findings.sort();
            for path in findings {
                ctx.installable_diagnostic(
                    installable,
                    Diagnostic::warn("")
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
}

/// Commands named by the executables in the user and admin directories. With
/// `strip_suffix`, the name stops at its first `.` that has characters after
/// it (lintian's `fileparse` with `\..+$`), as for a reliant's commands.
fn commands_in(files: &FileTree, strip_suffix: bool) -> HashSet<String> {
    let mut commands = HashSet::new();
    for entry in files.entries() {
        if !(entry.is_file() || entry.is_symlink()) {
            continue;
        }
        let Some(parent) = entry.path().parent().and_then(|parent| parent.to_str()) else {
            continue;
        };
        if !USER_DIRS.contains(&parent) && !ADMIN_DIRS.contains(&parent) {
            continue;
        }
        let Some(name) = entry.path().file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let name = if strip_suffix {
            match name.find('.') {
                Some(index) if index + 1 < name.len() => &name[..index],
                _ => name,
            }
        } else {
            name
        };
        commands.insert(name.to_string());
    }
    commands
}

/// The manual pages of section 1 or 8 whose command none of `related` covers,
/// by path.
fn surplus_pages(files: &FileTree, related: &HashSet<String>) -> Vec<PathBuf> {
    let mut pages: HashMap<String, Vec<(PathBuf, char)>> = HashMap::new();
    for entry in files.entries() {
        let Some(page) = section_page(entry) else {
            continue;
        };
        let Some(command) = command_from_manual_name(page.file_name) else {
            continue;
        };
        let Some(section) = directory_section(&page.page_path) else {
            continue;
        };
        pages
            .entry(command)
            .or_default()
            .push((entry.path().to_path_buf(), section));
    }
    let mut surplus = Vec::new();
    for (command, found) in &pages {
        if related.contains(command) || related.iter().any(|known| covers(known, command)) {
            continue;
        }
        for (path, section) in found {
            if *section == '1' || *section == '8' {
                surplus.push(path.clone());
            }
        }
    }
    surplus
}

/// Lintian's `grep { !/^\Q$command\E(?:\b|_)/ }`: the page's command starts
/// with `command`, then a `_`, or a word boundary.
fn covers(command: &str, page: &str) -> bool {
    let Some(rest) = page.strip_prefix(command) else {
        return false;
    };
    if rest.starts_with('_') {
        return true;
    }
    let before = command.chars().next_back().is_some_and(is_word);
    let after = rest.chars().next().is_some_and(is_word);
    before != after
}

fn is_word(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

/// The digit of the `manN` directory a page lives in; `page_path` ends with a
/// slash, as [`section_page`] gives it.
fn directory_section(page_path: &str) -> Option<char> {
    let directory = page_path.strip_suffix('/')?.rsplit('/').next()?;
    directory
        .strip_prefix("man")
        .and_then(|digits| digits.chars().next())
        .filter(char::is_ascii_digit)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{GroupCase, GroupTester};

    use super::*;

    const DSC: &str = "Source: example\nVersion: 1.0\n";

    /// A package with the given files (path, content).
    fn package(name: &str, files: &[(&str, &str)]) -> GroupCase {
        GroupCase::new(DSC)
            .installable(name, "")
            .files(files.to_vec())
    }

    #[test]
    fn group() {
        let pass = vec![
            // The page documents a command of the package.
            package(
                "foo",
                &[("usr/bin/foo", "x"), ("usr/share/man/man1/foo.1.gz", "x")],
            ),
            // Admin command, a section 8 page, another locale.
            package(
                "foo",
                &[
                    ("usr/sbin/foo", "x"),
                    ("usr/share/man/de/man8/foo.8.gz", "x"),
                ],
            ),
            // Other sections are not looked at.
            package("foo", &[("usr/share/man/man5/foo.5.gz", "x")]),
            // A udeb is skipped.
            GroupCase::new(DSC)
                .udeb("foo-udeb", "")
                .files([("usr/share/man/man1/foo.1.gz", "x")]),
            // A page a word boundary or `_` after its command covers.
            package(
                "foo",
                &[
                    ("usr/bin/foo", "x"),
                    ("usr/share/man/man1/foo-bar.1.gz", "x"),
                    ("usr/share/man/man1/foo_baz.1.gz", "x"),
                ],
            ),
            // A strong reliant ships the command, with a suffix.
            GroupCase::new(DSC)
                .installable("libfoo", "")
                .files([("usr/share/man/man1/foo.1.gz", "x")])
                .installable("foo-bin", "Depends: libfoo\n")
                .files([("usr/bin/foo.sh", "x")]),
        ];
        let fail = vec![
            package("foo", &[("usr/share/man/man1/foo.1.gz", "x")]),
            // A lone .deb is checked too: lintian's per-binary check has no `.dsc`.
            GroupCase::without_dsc()
                .installable("foo", "")
                .files([("usr/share/man/man1/foo.1.gz", "x")]),
            package("foo", &[("usr/share/man/man8/foo.8.gz", "x")]),
            package("foo", &[("usr/share/man/de/man1/foo.1.gz", "x")]),
            // A command outside the user and admin directories does not count.
            package(
                "foo",
                &[("usr/share/foo", "x"), ("usr/share/man/man1/foo.1.gz", "x")],
            ),
            // The page's command must be followed by a boundary.
            package(
                "foo",
                &[
                    ("usr/bin/foo", "x"),
                    ("usr/share/man/man1/foobar.1.gz", "x"),
                ],
            ),
            // A weak reliant does not count.
            GroupCase::new(DSC)
                .installable("libfoo", "")
                .files([("usr/share/man/man1/foo.1.gz", "x")])
                .installable("foo-bin", "Recommends: libfoo\n")
                .files([("usr/bin/foo", "x")]),
        ];
        GroupTester::new(SpareManualPage, pass, fail).test_and_snapshot();
    }
}
