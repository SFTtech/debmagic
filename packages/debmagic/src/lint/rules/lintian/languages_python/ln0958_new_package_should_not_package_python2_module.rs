use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::debian_changelog::DebianChangelog;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::gzip::gunzip;
use crate::lint::rule::BinaryPackageRule;

pub struct NewPackageShouldNotPackagePython2Module;

declare_rule! {
    /// A new `python-*` or `python2-*` package that only has one changelog
    /// entry, and whose entry does not say it is a Python 2 variant or
    /// version, is reported. The hint extra is the package name.
    ///
    /// The installed changelog (`changelog.Debian[.gz]`, `changelog[.gz]`
    /// under `usr/share/doc/<package>`) is read; a symlink there is followed
    /// one hop to a name in the same directory. Packages whose name ends in
    /// `-dev`, `-doc`, `-docs`, `-common` or `-tools` are skipped. A Changes
    /// text that mentions the package name also clears the hint.
    NewPackageShouldNotPackagePython2Module,
    code = "LN0958",
    tag = "new-package-should-not-package-python2-module",
    default_selected = true,
    experimental = false,
}

const CHANGELOG_CANDIDATES: &[&str] = &[
    "changelog.Debian.gz",
    "changelog.Debian",
    "changelog.debian.gz",
    "changelog.debian",
    "changelog.gz",
    "changelog",
];

static PYTHON2_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^python2?-").expect("valid regex"));
static IGNORED_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(-dev$|-docs?$|-common$|-tools$)").expect("valid regex"));
static PYTHON2_VARIANT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bpython ?2(?:\.x)? (?:variant|version)\b").expect("valid regex")
});

impl BinaryPackageRule for NewPackageShouldNotPackagePython2Module {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        if !PYTHON2_NAME.is_match(&package) || IGNORED_SUFFIX.is_match(&package) {
            return;
        }
        let Some(entries) = installed_changelog_entries(ctx.data_files(), &package) else {
            return;
        };
        let [entry] = entries.as_slice() else {
            return;
        };
        let changes = entry.changes();
        if PYTHON2_VARIANT.is_match(changes) || changes.contains(&package) {
            return;
        }
        ctx.diagnostic(Diagnostic::warn(package));
    }
}

/// The entries of the installed Debian changelog, parsed. `None` when there
/// is no such file or it cannot be read.
fn installed_changelog_entries(
    files: Option<&FileTree>,
    package: &str,
) -> Option<Vec<crate::lint::debian_changelog::ChangelogEntry>> {
    let files = files?;
    let dir = format!("usr/share/doc/{package}");
    let entry = CHANGELOG_CANDIDATES.iter().find_map(|name| {
        let entry = files.get(PathBuf::from(format!("{dir}/{name}")))?;
        matches!(
            entry.kind(),
            EntryKind::RegularFile | EntryKind::Hardlink | EntryKind::Symlink
        )
        .then_some(entry)
    })?;
    let resolved = if entry.kind() == EntryKind::Symlink {
        let target = entry.link_target()?;
        files.get(PathBuf::from(&dir).join(target))?
    } else {
        entry
    };
    if !matches!(
        resolved.kind(),
        EntryKind::RegularFile | EntryKind::Hardlink
    ) {
        return None;
    }
    let bytes = files.read(resolved).ok()?;
    let plain = if resolved
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".gz"))
    {
        gunzip(&bytes)?
    } else {
        bytes
    };
    let text = String::from_utf8(plain).ok()?;
    Some(DebianChangelog::parse(&text).entries().to_vec())
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    const ONE_ENTRY: &str = "python-foo (1.0-1) unstable; urgency=low\n\n  * Initial release.\n\n -- Maintainer <maint@example.org>  Mon, 01 Jan 1990 00:00:00 +0000\n";
    const PYTHON2_ENTRY: &str = "python-foo (1.0-1) unstable; urgency=low\n\n  * Packaging the python2 version.\n\n -- Maintainer <maint@example.org>  Mon, 01 Jan 1990 00:00:00 +0000\n";
    const TWO_ENTRIES: &str = "python-foo (1.0-2) unstable; urgency=low\n\n  * Second upload.\n\n -- Maintainer <maint@example.org>  Mon, 01 Jan 1990 00:00:00 +0000\n\npython-foo (1.0-1) unstable; urgency=low\n\n  * Initial release.\n\n -- Maintainer <maint@example.org>  Mon, 01 Jan 1990 00:00:00 +0000\n";

    fn with_changelog(package: &str, changelog: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![
            (
                format!("usr/share/doc/{package}/changelog.Debian"),
                TestFile::new(changelog),
            ),
            (
                "usr/lib/python3/dist-packages/foo.py".to_string(),
                TestFile::new("x"),
            ),
        ])
        .package(package)
    }

    #[test]
    fn binary() {
        let pass = vec![
            // Two entries: the package is not new.
            with_changelog("python-foo", TWO_ENTRIES),
            // The entry says it is a Python 2 variant.
            with_changelog("python-foo", PYTHON2_ENTRY),
            // Not a Python package name.
            with_changelog("foo-python", ONE_ENTRY),
            // An ignored suffix.
            with_changelog("python-foo-doc", ONE_ENTRY),
            // No changelog at all.
            BinaryPackageCase::from(vec![(
                "usr/lib/python3/dist-packages/foo.py".to_string(),
                TestFile::new("x"),
            )])
            .package("python-foo"),
        ];
        let fail = vec![
            with_changelog("python-foo", ONE_ENTRY),
            with_changelog("python2-foo", ONE_ENTRY),
        ];
        BinaryPackageTester::new(NewPackageShouldNotPackagePython2Module, pass, fail)
            .test_and_snapshot();
    }
}
