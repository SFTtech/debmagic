//! Paths Debian added, for the `debian-adds-` Tags in `files/artifact`.
//!
//! Source tree (`orig_files()` is `None`): every matching path in `files()`.
//! Source package: patched paths whose stored path is not in orig. An empty
//! orig emits nothing when the package is native (no Dsc, `3.0 (native)`, or
//! `1.0` with no `.orig.tar` in `Files`). A non-native package can still have
//! an empty orig, and then every patched path was added. `3.0 (quilt)` (Dsc
//! `Format`, otherwise `debian/source/format`; a missing format is `1.0`)
//! drops paths matching `^.pc/` before the patterns, which is why quilt's own
//! `.pc` does not fire.
//!
//! Directory names are matched with the trailing slash lintian stores.
//! `first_value` order is the Perl array order, so one path emits one Tag.
//! `source-contains-` and `no-debian-changes` are not implemented.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use debmagic_common::debian::source::SourceFormat;
use regex::Regex;

use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Artifact {
    ArchControlDir,
    ArchInventoryFile,
    BtsControlDir,
    BzrControlDir,
    CvsConflictCopy,
    CvsControlDir,
    EditorBackupFile,
    GitControlDir,
    HgControlDir,
    HgTagsFile,
    PatchFailureFile,
    SvkCommitFile,
    SvnCommitFile,
    SvnConflictFile,
    SvnControlDir,
    QuiltControlDir,
}

/// Perl `/pattern/s`: `(?s)` is the `/s` flag, so `.` matches a newline.
fn perl_regex(pattern: &str) -> Regex {
    let with_dotall = format!("(?s){pattern}");
    Regex::new(&with_dotall).unwrap_or_else(|error| panic!("{pattern} is not a regex: {error}"))
}

static DIRECTORY_CHECKS: LazyLock<Vec<(Regex, Artifact)>> = LazyLock::new(|| {
    vec![
        (perl_regex(r"^(.+/)?CVS/?$"), Artifact::CvsControlDir),
        (perl_regex(r"^(.+/)?\.svn/?$"), Artifact::SvnControlDir),
        (perl_regex(r"^(.+/)?\.bzr/?$"), Artifact::BzrControlDir),
        (perl_regex(r"^(.+/)?\{arch\}/?$"), Artifact::ArchControlDir),
        (
            perl_regex(r"^(.+/)?\.arch-ids/?$"),
            Artifact::ArchControlDir,
        ),
        (perl_regex(r"^(.+/)?,,.+/?$"), Artifact::ArchControlDir),
        (perl_regex(r"^(.+/)?\.git/?$"), Artifact::GitControlDir),
        (perl_regex(r"^(.+/)?\.hg/?$"), Artifact::HgControlDir),
        (perl_regex(r"^(.+/)?\.be/?$"), Artifact::BtsControlDir),
        (perl_regex(r"^(.+/)?\.ditrack/?$"), Artifact::BtsControlDir),
        (perl_regex(r"^(.+/)?\.pc/?$"), Artifact::QuiltControlDir),
    ]
});

static FILE_CHECKS: LazyLock<Vec<(Regex, Artifact)>> = LazyLock::new(|| {
    vec![
        (
            perl_regex(r"^(.+/)?svn-commit\.(.+\.)?tmp$"),
            Artifact::SvnCommitFile,
        ),
        (
            perl_regex(r"^(.+/)?svk-commit.+\.tmp$"),
            Artifact::SvkCommitFile,
        ),
        (
            perl_regex(r"^(.+/)?\.arch-inventory$"),
            Artifact::ArchInventoryFile,
        ),
        (perl_regex(r"^(.+/)?\.hgtags$"), Artifact::HgTagsFile),
        (
            perl_regex(r"^(.+/)?\.#(.+?)\.\d+(\.\d+)*$"),
            Artifact::CvsConflictCopy,
        ),
        (
            perl_regex(r"^(.+/)?(.+?)\.(r[1-9]\d*)$"),
            Artifact::SvnConflictFile,
        ),
        (perl_regex(r"\.(orig|rej)$"), Artifact::PatchFailureFile),
        (
            perl_regex(r"((^|/)[^/]+\.swp|~)$"),
            Artifact::EditorBackupFile,
        ),
    ]
});

/// Perl `m{^.pc/}` with no `/s`. `.` is any character except a newline.
static QUILT_UNPACK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^.pc/").expect("valid regex"));

pub(super) fn flag_added(ctx: &mut SourceContext<'_>, want: Artifact) {
    let mut paths: Vec<PathBuf> = added_by_debian(ctx)
        .into_iter()
        .filter(|(name, kind)| classify(name, *kind) == Some(want))
        .map(|(name, _)| PathBuf::from(name))
        .collect();
    paths.sort();
    for path in paths {
        ctx.diagnostic(Diagnostic::warn("").with_location(Location {
            path,
            line: None,
            column: None,
        }));
    }
}

fn added_by_debian(ctx: &mut SourceContext<'_>) -> Vec<(String, EntryKind)> {
    let empty_orig = match ctx.orig_files() {
        None => false,
        Some(orig) => orig.entries().is_empty(),
    };
    if empty_orig && package_is_native(ctx) {
        return Vec::new();
    }
    let orig_paths = ctx.orig_files().map(stored_paths);
    let quilt = orig_paths.is_some() && is_quilt(ctx);
    let Some(patched) = ctx.files() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in patched.entries() {
        let kind = entry.kind();
        if !is_checked_kind(kind) {
            continue;
        }
        if orig_paths
            .as_ref()
            .is_some_and(|orig| orig.contains(entry.path()))
        {
            continue;
        }
        let Some(name) = lintian_name(entry.path(), kind) else {
            continue;
        };
        if quilt && QUILT_UNPACK.is_match(&name) {
            continue;
        }
        found.push((name, kind));
    }
    found
}

fn is_checked_kind(kind: EntryKind) -> bool {
    matches!(
        kind,
        EntryKind::Directory | EntryKind::RegularFile | EntryKind::Hardlink
    )
}

fn stored_paths(tree: &FileTree) -> HashSet<PathBuf> {
    tree.entries()
        .iter()
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

/// No Dsc is the native unit-test fixture. `3.0 (native)` is native. `1.0`
/// is native when `Files` names no `.orig.tar`; a non-native format is not,
/// even when the orig tree itself is empty.
fn package_is_native(ctx: &SourceContext<'_>) -> bool {
    let Some(dsc) = ctx.dsc() else {
        return true;
    };
    let format = dsc.fields().get("Format").map(str::trim).unwrap_or("1.0");
    match format {
        "3.0 (native)" => true,
        "3.0 (quilt)" | "2.0" | "3.0 (git)" | "3.0 (bzr)" | "3.0 (custom)" => false,
        _ => {
            let files = dsc.fields().get("Files").unwrap_or("");
            !files
                .split_whitespace()
                .any(|part| part.contains(".orig.tar"))
        }
    }
}

/// Dsc `Format` when that field is present, otherwise `debian/source/format`.
fn is_quilt(ctx: &mut SourceContext<'_>) -> bool {
    if let Some(dsc) = ctx.dsc()
        && let Some(format) = dsc.fields().get("Format")
    {
        return SourceFormat::parse(format) == SourceFormat::Quilt;
    }
    let Some(files) = ctx.files() else {
        return false;
    };
    let Some(entry) = files.get(Path::new("debian/source/format")) else {
        return false;
    };
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return false;
    }
    let Ok(bytes) = files.read(entry) else {
        return false;
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return false;
    };
    SourceFormat::parse(text) == SourceFormat::Quilt
}

/// Directory index names end in `/`, which is the form the Perl regexes see.
fn lintian_name(path: &Path, kind: EntryKind) -> Option<String> {
    let raw = path.to_str()?;
    if kind == EntryKind::Directory && !raw.ends_with('/') {
        Some(format!("{raw}/"))
    } else {
        Some(raw.to_string())
    }
}

fn classify(name: &str, kind: EntryKind) -> Option<Artifact> {
    let checks = match kind {
        EntryKind::Directory => DIRECTORY_CHECKS.as_slice(),
        EntryKind::RegularFile | EntryKind::Hardlink => FILE_CHECKS.as_slice(),
        EntryKind::Symlink => return None,
    };
    checks
        .iter()
        .find(|(pattern, _)| pattern.is_match(name))
        .map(|(_, hit)| *hit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_follows_perl_order() {
        let cases = [
            (
                ".#foo.r1",
                EntryKind::RegularFile,
                Some(Artifact::SvnConflictFile),
            ),
            (
                "debian/.#foo.1.1",
                EntryKind::RegularFile,
                Some(Artifact::CvsConflictCopy),
            ),
            (
                ".#foo.1",
                EntryKind::Hardlink,
                Some(Artifact::CvsConflictCopy),
            ),
            (
                "foo.r1",
                EntryKind::RegularFile,
                Some(Artifact::SvnConflictFile),
            ),
            (
                "debian/foo.r1352",
                EntryKind::RegularFile,
                Some(Artifact::SvnConflictFile),
            ),
            (
                "a.r10",
                EntryKind::RegularFile,
                Some(Artifact::SvnConflictFile),
            ),
            ("foo.r0", EntryKind::RegularFile, None),
            ("foo.r01", EntryKind::RegularFile, None),
            (".r1", EntryKind::RegularFile, None),
            (
                "svn-commit.tmp",
                EntryKind::RegularFile,
                Some(Artifact::SvnCommitFile),
            ),
            (
                "debian/svn-commit.msg.tmp",
                EntryKind::RegularFile,
                Some(Artifact::SvnCommitFile),
            ),
            ("svk-commit.tmp", EntryKind::RegularFile, None),
            (
                "svk-commit444.tmp",
                EntryKind::RegularFile,
                Some(Artifact::SvkCommitFile),
            ),
            (
                "debian/svk-commit444.tmp",
                EntryKind::Hardlink,
                Some(Artifact::SvkCommitFile),
            ),
            (
                ".arch-inventory",
                EntryKind::RegularFile,
                Some(Artifact::ArchInventoryFile),
            ),
            (
                ".hgtags",
                EntryKind::RegularFile,
                Some(Artifact::HgTagsFile),
            ),
            (
                "foo.orig",
                EntryKind::RegularFile,
                Some(Artifact::PatchFailureFile),
            ),
            (
                "debian/foo.rej",
                EntryKind::RegularFile,
                Some(Artifact::PatchFailureFile),
            ),
            (
                "foo~.orig",
                EntryKind::RegularFile,
                Some(Artifact::PatchFailureFile),
            ),
            ("foo.original", EntryKind::RegularFile, None),
            ("foo.ORIG", EntryKind::RegularFile, None),
            (
                "foo.orig~",
                EntryKind::RegularFile,
                Some(Artifact::EditorBackupFile),
            ),
            (
                "foo~",
                EntryKind::RegularFile,
                Some(Artifact::EditorBackupFile),
            ),
            (
                "debian/foo~",
                EntryKind::RegularFile,
                Some(Artifact::EditorBackupFile),
            ),
            (
                "dir/a.swp",
                EntryKind::RegularFile,
                Some(Artifact::EditorBackupFile),
            ),
            (".swp", EntryKind::RegularFile, None),
            ("foo~", EntryKind::Directory, None),
            ("foo~", EntryKind::Symlink, None),
            ("CVS", EntryKind::RegularFile, None),
            ("CVS/", EntryKind::Directory, Some(Artifact::CvsControlDir)),
            ("cvs/", EntryKind::Directory, None),
            (
                "debian/CVS/",
                EntryKind::Directory,
                Some(Artifact::CvsControlDir),
            ),
            (".svn/", EntryKind::Directory, Some(Artifact::SvnControlDir)),
            (".bzr/", EntryKind::Directory, Some(Artifact::BzrControlDir)),
            (
                "{arch}/",
                EntryKind::Directory,
                Some(Artifact::ArchControlDir),
            ),
            (
                "debian/{arch}/",
                EntryKind::Directory,
                Some(Artifact::ArchControlDir),
            ),
            (
                ".arch-ids/",
                EntryKind::Directory,
                Some(Artifact::ArchControlDir),
            ),
            (
                ",,trace/",
                EntryKind::Directory,
                Some(Artifact::ArchControlDir),
            ),
            (",,/", EntryKind::Directory, Some(Artifact::ArchControlDir)),
            (
                ",,.git/",
                EntryKind::Directory,
                Some(Artifact::ArchControlDir),
            ),
            (".git/", EntryKind::Directory, Some(Artifact::GitControlDir)),
            (
                "debian/.git/",
                EntryKind::Directory,
                Some(Artifact::GitControlDir),
            ),
            (".git", EntryKind::Directory, Some(Artifact::GitControlDir)),
            (".github/", EntryKind::Directory, None),
            (".hg/", EntryKind::Directory, Some(Artifact::HgControlDir)),
            (".hgtags/", EntryKind::Directory, None),
            (".be/", EntryKind::Directory, Some(Artifact::BtsControlDir)),
            (
                ".ditrack/",
                EntryKind::Directory,
                Some(Artifact::BtsControlDir),
            ),
            (".before/", EntryKind::Directory, None),
            (
                ".pc/",
                EntryKind::Directory,
                Some(Artifact::QuiltControlDir),
            ),
            (
                "debian/.pc/",
                EntryKind::Directory,
                Some(Artifact::QuiltControlDir),
            ),
            (".pc/applied/", EntryKind::Directory, None),
        ];
        for (name, kind, expected) in cases {
            assert_eq!(classify(name, kind), expected, "{name} {kind:?}");
        }
    }

    #[test]
    fn quilt_unpack_filter_is_anchored() {
        assert!(QUILT_UNPACK.is_match(".pc/"));
        assert!(QUILT_UNPACK.is_match(".pc/applied"));
        assert!(QUILT_UNPACK.is_match(".pc/.git/"));
        assert!(!QUILT_UNPACK.is_match("debian/.pc/"));
        assert!(!QUILT_UNPACK.is_match(".pc"));
    }
}
