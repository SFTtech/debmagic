//! Paths Debian added, for the `debian-adds-` Tags in `files/artifact`.
//!
//! Source tree (`orig_files()` is `None`): every matching path in `files()`.
//! Source package: patched paths whose stored path is not in orig. An empty
//! orig emits nothing when the package is native (no Dsc, `3.0 (native)`, or
//! `1.0` with no `.orig.tar` in `Files`). A non-native package can still have
//! an empty orig, and then every patched path was added. `3.0 (quilt)` (Dsc
//! `Format`, otherwise `debian/source/format`; a missing format is `1.0`)
//! drops paths under `.pc/` before the patterns, which is why quilt's own
//! `.pc` does not fire.
//!
//! Directory names are matched with the trailing slash lintian stores.
//! `first_value` order is the Perl array order, so one path emits one Tag.
//! The classification is computed once per Subject and shared by every
//! `debian-adds-` Rule. `source-contains-` and `no-debian-changes` are not
//! implemented.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use debmagic_common::debian::source::SourceFormat;

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

impl Artifact {
    const COUNT: usize = 16;

    fn index(self) -> usize {
        self as usize
    }
}

/// One classification of the paths Debian added, shared by every
/// `debian-adds-` Rule. Paths in a bucket are sorted.
struct AddedIndex {
    paths: [Vec<PathBuf>; Artifact::COUNT],
}

impl AddedIndex {
    fn push(&mut self, artifact: Artifact, path: PathBuf) {
        self.paths[artifact.index()].push(path);
    }
}

impl Default for AddedIndex {
    fn default() -> Self {
        Self {
            paths: std::array::from_fn(|_| Vec::new()),
        }
    }
}

pub(super) fn flag_added(ctx: &mut SourceContext<'_>, want: Artifact) {
    // Resolve the trees first. The classification below reads them too, and a
    // wait for the file walk stays on that walk's own span.
    let _ = ctx.files();
    let _ = ctx.orig_files();
    let index = ctx.shared_once(index_paths_debian_added);
    for path in &index.paths[want.index()] {
        ctx.diagnostic(Diagnostic::warn("").with_location(Location {
            path: path.clone(),
            line: None,
            column: None,
        }));
    }
}

fn index_paths_debian_added(ctx: &mut SourceContext<'_>) -> AddedIndex {
    let empty_orig = match ctx.orig_files() {
        None => false,
        Some(orig) => orig.entries().is_empty(),
    };
    if empty_orig && package_is_native(ctx) {
        return AddedIndex::default();
    }
    let orig_paths = ctx.orig_files().map(stored_paths);
    let quilt = orig_paths.is_some() && is_quilt(ctx);
    let Some(patched) = ctx.files() else {
        return AddedIndex::default();
    };
    let mut index = AddedIndex::default();
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
        let Some(name) = entry.path().to_str() else {
            continue;
        };
        if quilt && quilt_unpack_path(name, kind) {
            continue;
        }
        let Some(hit) = classify(name, kind) else {
            continue;
        };
        index.push(hit, reported_path(name, kind));
    }
    for paths in &mut index.paths {
        paths.sort();
    }
    index
}

/// Directory locations keep the trailing slash lintian stores.
fn reported_path(name: &str, kind: EntryKind) -> PathBuf {
    if kind == EntryKind::Directory && !name.ends_with('/') {
        PathBuf::from(format!("{name}/"))
    } else {
        PathBuf::from(name)
    }
}

/// Perl `m{^.pc/}`: the stored directory `.pc` is lintian's `.pc/`.
fn quilt_unpack_path(name: &str, kind: EntryKind) -> bool {
    name.starts_with(".pc/") || (kind == EntryKind::Directory && name == ".pc")
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

/// Perl directory patterns, in array order. A directory path may already end
/// in `/` (the form the regexes see) or be the stored path, which does not.
fn classify(name: &str, kind: EntryKind) -> Option<Artifact> {
    match kind {
        EntryKind::Directory => classify_directory(name),
        EntryKind::RegularFile | EntryKind::Hardlink => classify_file(name),
        EntryKind::Symlink => None,
    }
}

fn classify_directory(name: &str) -> Option<Artifact> {
    let base = directory_basename(name);
    if base == "CVS" {
        return Some(Artifact::CvsControlDir);
    }
    if base == ".svn" {
        return Some(Artifact::SvnControlDir);
    }
    if base == ".bzr" {
        return Some(Artifact::BzrControlDir);
    }
    if base == "{arch}" || base == ".arch-ids" {
        return Some(Artifact::ArchControlDir);
    }
    // `,,.+` matches when `,,` starts a component and at least one character
    // follows, including a trailing slash and the rest of the path. It is
    // checked before `.git`, so `,,foo/.git/` is arch, while `,,foo/CVS/` is
    // CVS because that pattern is earlier.
    if double_comma_directory(name) {
        return Some(Artifact::ArchControlDir);
    }
    if base == ".git" {
        return Some(Artifact::GitControlDir);
    }
    if base == ".hg" {
        return Some(Artifact::HgControlDir);
    }
    if base == ".be" || base == ".ditrack" {
        return Some(Artifact::BtsControlDir);
    }
    if base == ".pc" {
        return Some(Artifact::QuiltControlDir);
    }
    None
}

fn classify_file(name: &str) -> Option<Artifact> {
    let base = file_basename(name);
    if is_svn_commit(base) {
        return Some(Artifact::SvnCommitFile);
    }
    if is_svk_commit(base) {
        return Some(Artifact::SvkCommitFile);
    }
    if base == ".arch-inventory" {
        return Some(Artifact::ArchInventoryFile);
    }
    if base == ".hgtags" {
        return Some(Artifact::HgTagsFile);
    }
    if is_cvs_conflict(base) {
        return Some(Artifact::CvsConflictCopy);
    }
    if is_svn_conflict(base) {
        return Some(Artifact::SvnConflictFile);
    }
    if name.ends_with(".orig") || name.ends_with(".rej") {
        return Some(Artifact::PatchFailureFile);
    }
    if name.ends_with('~') || is_vim_swp(base) {
        return Some(Artifact::EditorBackupFile);
    }
    None
}

fn directory_basename(name: &str) -> &str {
    file_basename(name.strip_suffix('/').unwrap_or(name))
}

fn file_basename(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

/// `^(.+/)?,,.+/?$`, plus the trailing slash lintian appends to a directory.
fn double_comma_directory(name: &str) -> bool {
    if component_starts_with_double_comma(name) {
        return true;
    }
    // Stored directory paths have no slash. Appending `/` gives `,,.+` the
    // character it requires when the component is exactly `,,`.
    !name.ends_with('/') && directory_basename(name) == ",,"
}

fn component_starts_with_double_comma(name: &str) -> bool {
    let mut rest = name;
    loop {
        if let Some(after) = rest.strip_prefix(",,") {
            return !after.is_empty();
        }
        match rest.find('/') {
            Some(index) => {
                rest = &rest[index + 1..];
                if rest.is_empty() {
                    return false;
                }
            }
            None => return false,
        }
    }
}

/// `svn-commit.tmp` or `svn-commit.<words>.tmp`.
fn is_svn_commit(base: &str) -> bool {
    let Some(rest) = base.strip_prefix("svn-commit.") else {
        return false;
    };
    if rest == "tmp" {
        return true;
    }
    matches!(rest.strip_suffix(".tmp"), Some(middle) if !middle.is_empty())
}

/// `svk-commit` plus at least one character, then `.tmp`.
fn is_svk_commit(base: &str) -> bool {
    let Some(rest) = base.strip_prefix("svk-commit") else {
        return false;
    };
    matches!(rest.strip_suffix(".tmp"), Some(middle) if !middle.is_empty())
}

/// `.#<name>.<digits>(.<digits>)*` with a non-empty name.
fn is_cvs_conflict(base: &str) -> bool {
    let Some(rest) = base.strip_prefix(".#") else {
        return false;
    };
    rest.match_indices('.')
        .any(|(index, _)| index > 0 && is_dot_digits_tail(&rest[index..]))
}

/// `\.\d+(\.\d+)*`
fn is_dot_digits_tail(tail: &str) -> bool {
    let mut rest = tail;
    if !rest.starts_with('.') {
        return false;
    }
    loop {
        let digits = rest[1..]
            .bytes()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
        if digits == 0 {
            return false;
        }
        rest = &rest[1 + digits..];
        if rest.is_empty() {
            return true;
        }
        if !rest.starts_with('.') {
            return false;
        }
    }
}

/// `<name>.r<revision>` with a non-empty name and no leading zero.
fn is_svn_conflict(base: &str) -> bool {
    let Some((head, tail)) = base.rsplit_once('.') else {
        return false;
    };
    if head.is_empty() {
        return false;
    }
    let Some(revision) = tail.strip_prefix('r') else {
        return false;
    };
    let mut digits = revision.bytes();
    matches!(digits.next(), Some(b'1'..=b'9')) && digits.all(|byte| byte.is_ascii_digit())
}

/// A non-empty basename ending in `.swp`. `.swp` itself does not match.
fn is_vim_swp(base: &str) -> bool {
    base.strip_suffix(".swp")
        .is_some_and(|head| !head.is_empty())
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
            (
                ",,foo/.git/",
                EntryKind::Directory,
                Some(Artifact::ArchControlDir),
            ),
            (
                ",,foo/CVS/",
                EntryKind::Directory,
                Some(Artifact::CvsControlDir),
            ),
            (
                "foo/,,/bar/",
                EntryKind::Directory,
                Some(Artifact::ArchControlDir),
            ),
            (",,", EntryKind::Directory, Some(Artifact::ArchControlDir)),
            ("svn-commit..tmp", EntryKind::RegularFile, None),
            (".#.1", EntryKind::RegularFile, None),
            (
                ".#.1.2",
                EntryKind::RegularFile,
                Some(Artifact::CvsConflictCopy),
            ),
        ];
        for (name, kind, expected) in cases {
            assert_eq!(classify(name, kind), expected, "{name} {kind:?}");
        }
    }

    #[test]
    fn quilt_unpack_filter_is_anchored() {
        assert!(quilt_unpack_path(".pc/", EntryKind::Directory));
        assert!(quilt_unpack_path(".pc", EntryKind::Directory));
        assert!(quilt_unpack_path(".pc/applied", EntryKind::RegularFile));
        assert!(quilt_unpack_path(".pc/.git/", EntryKind::Directory));
        assert!(!quilt_unpack_path("debian/.pc/", EntryKind::Directory));
        assert!(!quilt_unpack_path(".pc", EntryKind::RegularFile));
    }

    #[test]
    fn artifact_index_matches_declaration_order() {
        let all = [
            Artifact::ArchControlDir,
            Artifact::ArchInventoryFile,
            Artifact::BtsControlDir,
            Artifact::BzrControlDir,
            Artifact::CvsConflictCopy,
            Artifact::CvsControlDir,
            Artifact::EditorBackupFile,
            Artifact::GitControlDir,
            Artifact::HgControlDir,
            Artifact::HgTagsFile,
            Artifact::PatchFailureFile,
            Artifact::SvkCommitFile,
            Artifact::SvnCommitFile,
            Artifact::SvnConflictFile,
            Artifact::SvnControlDir,
            Artifact::QuiltControlDir,
        ];
        assert_eq!(all.len(), Artifact::COUNT);
        for (index, artifact) in all.into_iter().enumerate() {
            assert_eq!(artifact.index(), index);
        }
    }
}
