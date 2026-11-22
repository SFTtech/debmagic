use std::fs;
use std::os::unix::fs::{FileExt, FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError};

use anyhow::{Context, bail};
use globset::{Glob, GlobMatcher};
use regex::Regex;

use super::digest::{DigestAlgorithm, hex_digest};
use super::elf::entry_bytes;
use super::file_type::FileType;

/// Which variant an [`Entry`] is, without its data. For filtering and matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryKind {
    RegularFile,
    Directory,
    Symlink,
    Hardlink,
    /// Named pipe (tar type `p`).
    Fifo,
    /// Character device (tar type `c`).
    CharDevice,
    /// Block device (tar type `b`).
    BlockDevice,
}

/// Who owns an [`Entry`], as the tar header of a Binary package says.
///
/// An Entry on a File tree that has no archive header (a Source tree on disk,
/// a fixture that does not say) has no `Owner` at all: [`Entry::owner`] is
/// `None`, not `root/root`.
///
/// The names are what the header carries, which may be empty. Lintian's
/// `owner` and `group` are the name when there is one and else the number
/// (`tar tv` prints it so), except that id `0` is shown as `root`; so are
/// [`Owner::user`] and [`Owner::group`]. [`Owner::identity`] is lintian's
/// `identity`, `user/group`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner {
    uid: u64,
    gid: u64,
    user_name: Option<String>,
    group_name: Option<String>,
}

impl Owner {
    /// `user_name` and `group_name` are the header's names, `None` when the
    /// header has none (an empty name is none).
    pub fn new(uid: u64, gid: u64, user_name: Option<String>, group_name: Option<String>) -> Self {
        Self {
            uid,
            gid,
            user_name: user_name.filter(|name| !name.is_empty()),
            group_name: group_name.filter(|name| !name.is_empty()),
        }
    }

    /// `root/root`, ids `0` and `0`: what `dpkg-deb` and `dh_fixperms` give.
    pub fn root() -> Self {
        Self::named(0, 0, "root", "root")
    }

    /// Both ids and names.
    pub fn named(uid: u64, gid: u64, user: &str, group: &str) -> Self {
        Self::new(uid, gid, Some(user.to_string()), Some(group.to_string()))
    }

    /// Lintian's `uid`.
    #[allow(dead_code)]
    pub fn uid(&self) -> u64 {
        self.uid
    }

    /// Lintian's `gid`.
    #[allow(dead_code)]
    pub fn gid(&self) -> u64 {
        self.gid
    }

    /// Lintian's `owner`: the user name, else `root` for uid `0`, else the uid.
    pub fn user(&self) -> String {
        display_id(self.user_name.as_deref(), self.uid)
    }

    /// Lintian's `group`: the group name, else `root` for gid `0`, else the gid.
    pub fn group(&self) -> String {
        display_id(self.group_name.as_deref(), self.gid)
    }

    /// Lintian's `identity`: `user/group`, for example `root/games`.
    pub fn identity(&self) -> String {
        format!("{}/{}", self.user(), self.group())
    }
}

fn display_id(name: Option<&str>, id: u64) -> String {
    match name {
        Some(name) => name.to_string(),
        None if id == 0 => "root".to_string(),
        None => id.to_string(),
    }
}

/// A path in a File tree, with the data that variant has.
///
/// File `content` is `None` when the bytes live on the host and are read on
/// demand through [`FileTree::read`].
///
/// `mode` is the permission bits (`0o7777`: setuid, setgid, sticky, and
/// `rwx` for owner, group and other) as stored in the tar header or read from
/// host metadata. The file-type bits are not part of it; that is the variant.
///
/// `owner` is `None` unless the File tree knows it (see [`Owner`]). `mtime` is
/// seconds since the epoch from the tar header; `None` unless the File tree
/// knows it (see [`Entry::mtime`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    RegularFile {
        path: PathBuf,
        mode: u32,
        owner: Option<Owner>,
        mtime: Option<i64>,
        file_type: Option<FileType>,
        content: Option<Vec<u8>>,
    },
    /// `target` is the member the tar header links to, normalized like
    /// member paths (no `./` or `/` prefix). `None` on host File trees, where
    /// a file with more than one link has no distinguished original.
    Hardlink {
        path: PathBuf,
        mode: u32,
        owner: Option<Owner>,
        mtime: Option<i64>,
        file_type: Option<FileType>,
        content: Option<Vec<u8>>,
        target: Option<PathBuf>,
    },
    Directory {
        path: PathBuf,
        mode: u32,
        owner: Option<Owner>,
        mtime: Option<i64>,
    },
    /// `target` is exactly as stored, not resolved.
    Symlink {
        path: PathBuf,
        mode: u32,
        owner: Option<Owner>,
        mtime: Option<i64>,
        target: PathBuf,
    },
    /// A named pipe. Has no content to read.
    Fifo {
        path: PathBuf,
        mode: u32,
        owner: Option<Owner>,
        mtime: Option<i64>,
    },
    /// A character device node. Device numbers are not kept.
    CharDevice {
        path: PathBuf,
        mode: u32,
        owner: Option<Owner>,
        mtime: Option<i64>,
    },
    /// A block device node. Device numbers are not kept.
    BlockDevice {
        path: PathBuf,
        mode: u32,
        owner: Option<Owner>,
        mtime: Option<i64>,
    },
}

/// Permission bits an [`Entry`] keeps: setuid, setgid, sticky, and `rwx`.
const PERMISSION_BITS: u32 = 0o7777;

/// Any execute bit: owner, group, or other.
const EXECUTE_BITS: u32 = 0o111;

impl Entry {
    pub(crate) fn regular_file(
        path: impl Into<PathBuf>,
        file_type: Option<FileType>,
        content: Option<Vec<u8>>,
    ) -> Self {
        Self::RegularFile {
            path: path.into(),
            mode: 0o644,
            owner: None,
            mtime: None,
            file_type,
            content,
        }
    }

    pub(crate) fn hardlink(
        path: impl Into<PathBuf>,
        file_type: Option<FileType>,
        content: Option<Vec<u8>>,
    ) -> Self {
        Self::Hardlink {
            path: path.into(),
            mode: 0o644,
            owner: None,
            mtime: None,
            file_type,
            content,
            target: None,
        }
    }

    /// Set the path a hardlink links to. Other variants are returned as is.
    pub(crate) fn with_hardlink_target(mut self, new: impl Into<PathBuf>) -> Self {
        if let Self::Hardlink { target, .. } = &mut self {
            *target = Some(new.into());
        }
        self
    }

    pub(crate) fn directory(path: impl Into<PathBuf>) -> Self {
        Self::Directory {
            path: path.into(),
            mode: 0o755,
            owner: None,
            mtime: None,
        }
    }

    pub(crate) fn symlink(path: impl Into<PathBuf>, target: impl Into<PathBuf>) -> Self {
        Self::Symlink {
            path: path.into(),
            mode: 0o777,
            owner: None,
            mtime: None,
            target: target.into(),
        }
    }

    pub(crate) fn fifo(path: impl Into<PathBuf>) -> Self {
        Self::Fifo {
            path: path.into(),
            mode: 0o644,
            owner: None,
            mtime: None,
        }
    }

    pub(crate) fn char_device(path: impl Into<PathBuf>) -> Self {
        Self::CharDevice {
            path: path.into(),
            mode: 0o644,
            owner: None,
            mtime: None,
        }
    }

    pub(crate) fn block_device(path: impl Into<PathBuf>) -> Self {
        Self::BlockDevice {
            path: path.into(),
            mode: 0o644,
            owner: None,
            mtime: None,
        }
    }

    /// Set the permission bits; bits outside `0o7777` are dropped. Until this
    /// is called an Entry has the usual mode for its kind: `0644` for a file,
    /// hardlink, fifo or device, `0755` for a directory, `0777` for a symlink.
    pub(crate) fn with_mode(mut self, new: u32) -> Self {
        *self.mode_mut() = new & PERMISSION_BITS;
        self
    }

    /// Set the owner. Until this is called an Entry has none.
    pub(crate) fn with_owner(mut self, new: Owner) -> Self {
        *self.owner_mut() = Some(new);
        self
    }

    /// Set the modification time, seconds since the epoch; `None` leaves it
    /// unknown. Until this is called an Entry has none.
    pub(crate) fn with_mtime(mut self, new: Option<i64>) -> Self {
        *self.mtime_mut() = new;
        self
    }

    fn mtime_mut(&mut self) -> &mut Option<i64> {
        match self {
            Self::RegularFile { mtime, .. }
            | Self::Hardlink { mtime, .. }
            | Self::Directory { mtime, .. }
            | Self::Symlink { mtime, .. }
            | Self::Fifo { mtime, .. }
            | Self::CharDevice { mtime, .. }
            | Self::BlockDevice { mtime, .. } => mtime,
        }
    }

    fn mode_mut(&mut self) -> &mut u32 {
        match self {
            Self::RegularFile { mode, .. }
            | Self::Hardlink { mode, .. }
            | Self::Directory { mode, .. }
            | Self::Symlink { mode, .. }
            | Self::Fifo { mode, .. }
            | Self::CharDevice { mode, .. }
            | Self::BlockDevice { mode, .. } => mode,
        }
    }

    fn owner_mut(&mut self) -> &mut Option<Owner> {
        match self {
            Self::RegularFile { owner, .. }
            | Self::Hardlink { owner, .. }
            | Self::Directory { owner, .. }
            | Self::Symlink { owner, .. }
            | Self::Fifo { owner, .. }
            | Self::CharDevice { owner, .. }
            | Self::BlockDevice { owner, .. } => owner,
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            Self::RegularFile { path, .. }
            | Self::Hardlink { path, .. }
            | Self::Directory { path, .. }
            | Self::Symlink { path, .. }
            | Self::Fifo { path, .. }
            | Self::CharDevice { path, .. }
            | Self::BlockDevice { path, .. } => path,
        }
    }

    /// Permission bits, `0o7777` at most (setuid `0o4000`, setgid `0o2000`,
    /// sticky `0o1000`).
    pub fn mode(&self) -> u32 {
        match self {
            Self::RegularFile { mode, .. }
            | Self::Hardlink { mode, .. }
            | Self::Directory { mode, .. }
            | Self::Symlink { mode, .. }
            | Self::Fifo { mode, .. }
            | Self::CharDevice { mode, .. }
            | Self::BlockDevice { mode, .. } => *mode,
        }
    }

    /// Owner and group, when the File tree knows them: Entries of a Binary
    /// package (Data files and Control files) have them from the tar header.
    /// `None` on host File trees and on fixtures that set none; a Rule that
    /// needs an owner must decide what that means, it is not `root/root`.
    pub fn owner(&self) -> Option<&Owner> {
        match self {
            Self::RegularFile { owner, .. }
            | Self::Hardlink { owner, .. }
            | Self::Directory { owner, .. }
            | Self::Symlink { owner, .. }
            | Self::Fifo { owner, .. }
            | Self::CharDevice { owner, .. }
            | Self::BlockDevice { owner, .. } => owner.as_ref(),
        }
    }

    /// Modification time from the tar header, seconds since the epoch. `None`
    /// on host File trees and on fixtures that set none.
    pub fn mtime(&self) -> Option<i64> {
        match self {
            Self::RegularFile { mtime, .. }
            | Self::Hardlink { mtime, .. }
            | Self::Directory { mtime, .. }
            | Self::Symlink { mtime, .. }
            | Self::Fifo { mtime, .. }
            | Self::CharDevice { mtime, .. }
            | Self::BlockDevice { mtime, .. } => *mtime,
        }
    }

    /// The modification date as `YYYY-MM-DD` in UTC, lintian's `date`.
    /// `None` when [`Self::mtime`] is.
    pub fn mtime_date(&self) -> Option<String> {
        let date = chrono::DateTime::from_timestamp(self.mtime()?, 0)?;
        Some(date.format("%Y-%m-%d").to_string())
    }

    /// `true` when the owner is known and its [`Owner::identity`] is
    /// `identity` (`root/games`). Lintian's `$item->identity eq '...'`.
    pub fn has_identity(&self, identity: &str) -> bool {
        self.owner()
            .is_some_and(|owner| owner.identity() == identity)
    }

    /// A regular file or hardlink with any execute bit set. Lintian's
    /// `is_file && is_executable`.
    pub fn is_executable_file(&self) -> bool {
        self.is_file() && self.mode() & EXECUTE_BITS != 0
    }

    /// A regular file or a hardlink. Lintian's `is_file`.
    pub fn is_file(&self) -> bool {
        matches!(self, Self::RegularFile { .. } | Self::Hardlink { .. })
    }

    /// A regular file, a hardlink not included. Lintian's `is_regular_file`.
    #[allow(dead_code)]
    pub fn is_regular_file(&self) -> bool {
        matches!(self, Self::RegularFile { .. })
    }

    /// Lintian's `is_dir`.
    #[allow(dead_code)]
    pub fn is_dir(&self) -> bool {
        matches!(self, Self::Directory { .. })
    }

    /// Lintian's `is_symlink`.
    #[allow(dead_code)]
    pub fn is_symlink(&self) -> bool {
        matches!(self, Self::Symlink { .. })
    }

    /// Lintian's `is_hardlink`.
    #[allow(dead_code)]
    pub fn is_hardlink(&self) -> bool {
        matches!(self, Self::Hardlink { .. })
    }

    pub fn kind(&self) -> EntryKind {
        match self {
            Self::RegularFile { .. } => EntryKind::RegularFile,
            Self::Hardlink { .. } => EntryKind::Hardlink,
            Self::Directory { .. } => EntryKind::Directory,
            Self::Symlink { .. } => EntryKind::Symlink,
            Self::Fifo { .. } => EntryKind::Fifo,
            Self::CharDevice { .. } => EntryKind::CharDevice,
            Self::BlockDevice { .. } => EntryKind::BlockDevice,
        }
    }

    /// File type of a regular file or hardlink, when classifiable.
    pub fn file_type(&self) -> Option<FileType> {
        match self {
            Self::RegularFile { file_type, .. } | Self::Hardlink { file_type, .. } => *file_type,
            Self::Directory { .. }
            | Self::Symlink { .. }
            | Self::Fifo { .. }
            | Self::CharDevice { .. }
            | Self::BlockDevice { .. } => None,
        }
    }

    /// Where a symlink points. `None` for every other variant.
    pub fn link_target(&self) -> Option<&Path> {
        match self {
            Self::Symlink { target, .. } => Some(target),
            _ => None,
        }
    }

    /// The member a hardlink links to, as stored and normalized. `None` for
    /// every other variant and for hardlinks on host File trees.
    pub fn hardlink_target(&self) -> Option<&Path> {
        match self {
            Self::Hardlink { target, .. } => target.as_deref(),
            _ => None,
        }
    }
}

/// Entries a Rule can walk as if they sat on disk.
pub struct FileTree {
    #[allow(dead_code)]
    root: Option<PathBuf>,
    entries: Vec<Entry>,
    /// The MD5 of each Entry, by index in `entries`, computed on first ask.
    #[allow(dead_code)]
    md5sums: Vec<OnceLock<Option<String>>>,
}

impl FileTree {
    pub fn from_entries(root: Option<PathBuf>, mut entries: Vec<Entry>) -> Self {
        entries.sort_by(|left, right| left.path().cmp(right.path()));
        let md5sums = entries.iter().map(|_| OnceLock::new()).collect();
        Self {
            root,
            entries,
            md5sums,
        }
    }

    /// Lintian's `$item->md5sum`: the lowercase hex MD5 of a regular file's or
    /// hardlink's bytes. An Entry of this tree is digested once and the digest
    /// is shared by every Rule that asks. `None` for other kinds of Entry and
    /// for bytes that cannot be read.
    #[allow(dead_code)]
    pub(crate) fn md5sum(&self, entry: &Entry) -> Option<String> {
        if !entry.is_file() {
            return None;
        }
        let shared = self
            .entries
            .binary_search_by(|candidate| candidate.path().cmp(entry.path()))
            .ok()
            .filter(|&index| std::ptr::eq(&self.entries[index], entry));
        match shared {
            Some(index) => self.md5sums[index]
                .get_or_init(|| digest_md5(self, entry))
                .clone(),
            None => digest_md5(self, entry),
        }
    }

    /// Walk a directory on the host. The walk and the file classification both
    /// use as many threads as the current rayon pool has, so a lint run's
    /// thread count applies here too.
    pub fn from_host_directory(root: impl AsRef<Path>) -> anyhow::Result<Self> {
        let root = root.as_ref();
        let threads = rayon::current_num_threads();
        let (mut entries, pending) = collect_entries(root, threads)
            .with_context(|| format!("walking File tree at {}", root.display()))?;
        let classified = classify_files(&pending, threads)?;
        for (file, classified) in pending.iter().zip(classified) {
            set_file_type(&mut entries[file.index], classified);
        }
        Ok(Self::from_entries(Some(root.to_path_buf()), entries))
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Entry at `path`. The tree is sorted by path, so this is a binary search.
    pub fn get(&self, path: impl AsRef<Path>) -> Option<&Entry> {
        let path = path.as_ref();
        let index = self
            .entries
            .binary_search_by(|entry| entry.path().cmp(path))
            .ok()?;
        Some(&self.entries[index])
    }

    pub fn query(&self) -> Query<'_, 'static> {
        Query {
            tree: self,
            kinds: None,
            glob: None,
            path: None,
        }
    }

    pub fn read(&self, entry: &Entry) -> anyhow::Result<Vec<u8>> {
        let (Entry::RegularFile { content, .. } | Entry::Hardlink { content, .. }) = entry else {
            bail!("Entry {} is not a file", entry.path().display());
        };
        if let Some(bytes) = content {
            return Ok(bytes.clone());
        }
        let Some(root) = &self.root else {
            bail!(
                "Entry {} has no content and File tree has no directory root",
                entry.path().display()
            );
        };
        fs::read(root.join(entry.path()))
            .with_context(|| format!("reading {}", entry.path().display()))
    }
}

/// Filter over a File tree: Entry kind, path glob, and a path regex.
///
/// `'pat` is `'static` until [`Query::path`] borrows a [`Regex`].
pub struct Query<'a, 'pat> {
    tree: &'a FileTree,
    kinds: Option<Vec<EntryKind>>,
    glob: Option<GlobMatcher>,
    path: Option<&'pat Regex>,
}

impl<'a, 'pat> Query<'a, 'pat> {
    pub fn kinds(mut self, kinds: impl IntoIterator<Item = EntryKind>) -> Self {
        self.kinds = Some(kinds.into_iter().collect());
        self
    }

    #[allow(dead_code)]
    pub fn glob(mut self, pattern: &str) -> anyhow::Result<Self> {
        self.glob = Some(Glob::new(pattern)?.compile_matcher());
        Ok(self)
    }

    /// Keep entries whose stored path matches `pattern`.
    ///
    /// The match is [`Regex::is_match`] on the path as UTF-8, so the pattern
    /// is unanchored unless it anchors itself. A path that is not UTF-8 does
    /// not match. A later call replaces an earlier one.
    pub fn path<'p>(self, pattern: &'p Regex) -> Query<'a, 'p> {
        Query {
            tree: self.tree,
            kinds: self.kinds,
            glob: self.glob,
            path: Some(pattern),
        }
    }

    fn matches(&self, entry: &Entry) -> bool {
        if let Some(kinds) = &self.kinds
            && !kinds.contains(&entry.kind())
        {
            return false;
        }
        if let Some(glob) = &self.glob
            && !glob.is_match(entry.path())
        {
            return false;
        }
        if let Some(pattern) = self.path {
            match entry.path().to_str() {
                Some(name) if pattern.is_match(name) => {}
                _ => return false,
            }
        }
        true
    }
}

impl<'a, 'pat> IntoIterator for Query<'a, 'pat> {
    type Item = &'a Entry;
    type IntoIter = std::vec::IntoIter<&'a Entry>;

    fn into_iter(self) -> Self::IntoIter {
        self.tree
            .entries
            .iter()
            .filter(|entry| self.matches(entry))
            .collect::<Vec<_>>()
            .into_iter()
    }
}

/// A file found by the walk, to be classified once the walk is done.
struct PendingFile {
    /// Index of its entry in the entries being built.
    index: usize,
    host_path: PathBuf,
}

/// What one walker thread found: an entry per path, and for files the host
/// path to classify.
type Found = Vec<(Entry, Option<PathBuf>)>;

/// Walk `root` with up to `threads` threads, returning an entry per path
/// (unordered; [`FileTree::from_entries`] sorts them). Files get no file type
/// yet; they are returned in `pending` so they can be classified in parallel.
///
/// Every path is listed: no hidden-file, ignore-file or VCS filtering, and
/// symlinks are not followed. Plain `std::thread` workers, not rayon: this
/// runs inside a lazy initializer that other Rules wait on (see
/// [`classify_files`]).
fn collect_entries(root: &Path, threads: usize) -> anyhow::Result<(Vec<Entry>, Vec<PendingFile>)> {
    let found: Mutex<Vec<Found>> = Mutex::new(Vec::new());
    let first_error: Mutex<Option<anyhow::Error>> = Mutex::new(None);

    ignore::WalkBuilder::new(root)
        .standard_filters(false)
        .follow_links(false)
        .threads(threads)
        .build_parallel()
        .visit(&mut WalkCollectorBuilder {
            root,
            found: &found,
            first_error: &first_error,
        });

    if let Some(error) = first_error
        .into_inner()
        .unwrap_or_else(PoisonError::into_inner)
    {
        return Err(error);
    }
    let mut entries = Vec::new();
    let mut pending = Vec::new();
    for (entry, host_path) in found
        .into_inner()
        .unwrap_or_else(PoisonError::into_inner)
        .into_iter()
        .flatten()
    {
        if let Some(host_path) = host_path {
            pending.push(PendingFile {
                index: entries.len(),
                host_path,
            });
        }
        entries.push(entry);
    }
    Ok((entries, pending))
}

struct WalkCollectorBuilder<'s> {
    root: &'s Path,
    found: &'s Mutex<Vec<Found>>,
    first_error: &'s Mutex<Option<anyhow::Error>>,
}

impl<'s> ignore::ParallelVisitorBuilder<'s> for WalkCollectorBuilder<'s> {
    fn build(&mut self) -> Box<dyn ignore::ParallelVisitor + 's> {
        Box::new(WalkCollector {
            root: self.root,
            found: self.found,
            first_error: self.first_error,
            local: Vec::new(),
        })
    }
}

/// One walker thread's state; hands what it found to the shared list when the
/// walk is done with it.
struct WalkCollector<'s> {
    root: &'s Path,
    found: &'s Mutex<Vec<Found>>,
    first_error: &'s Mutex<Option<anyhow::Error>>,
    local: Found,
}

impl WalkCollector<'_> {
    fn entry_for(
        &self,
        result: Result<ignore::DirEntry, ignore::Error>,
    ) -> anyhow::Result<Option<(Entry, Option<PathBuf>)>> {
        let dir_entry = result.context("walking the directory failed")?;
        if dir_entry.depth() == 0 {
            return Ok(None);
        }
        let path = dir_entry.path();
        let relative = path
            .strip_prefix(self.root)
            .with_context(|| format!("relativizing {}", path.display()))?
            .to_path_buf();
        let metadata =
            fs::symlink_metadata(path).with_context(|| format!("stat {}", path.display()))?;
        let Some(entry) = host_entry(path, relative, &metadata)? else {
            return Ok(None);
        };
        let host_path = matches!(entry, Entry::RegularFile { .. } | Entry::Hardlink { .. })
            .then(|| path.to_path_buf());
        Ok(Some((entry, host_path)))
    }
}

impl ignore::ParallelVisitor for WalkCollector<'_> {
    fn visit(&mut self, result: Result<ignore::DirEntry, ignore::Error>) -> ignore::WalkState {
        match self.entry_for(result) {
            Ok(Some(found)) => {
                self.local.push(found);
                ignore::WalkState::Continue
            }
            Ok(None) => ignore::WalkState::Continue,
            Err(error) => {
                self.first_error
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .get_or_insert(error);
                ignore::WalkState::Quit
            }
        }
    }
}

impl Drop for WalkCollector<'_> {
    fn drop(&mut self) {
        let local = std::mem::take(&mut self.local);
        self.found
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(local);
    }
}

fn host_entry(
    path: &Path,
    relative: PathBuf,
    metadata: &fs::Metadata,
) -> anyhow::Result<Option<Entry>> {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        let target =
            fs::read_link(path).with_context(|| format!("reading symlink {}", path.display()))?;
        return Ok(Some(
            Entry::symlink(relative, target).with_mode(metadata.mode()),
        ));
    }
    if file_type.is_dir() {
        return Ok(Some(Entry::directory(relative).with_mode(metadata.mode())));
    }
    if file_type.is_file() {
        let entry = if metadata.nlink() > 1 {
            Entry::hardlink(relative, None, None)
        } else {
            Entry::regular_file(relative, None, None)
        };
        return Ok(Some(entry.with_mode(metadata.mode())));
    }
    // Sockets cannot be in a tar archive, so they are not listed.
    let special = if file_type.is_fifo() {
        Entry::fifo(relative)
    } else if file_type.is_char_device() {
        Entry::char_device(relative)
    } else if file_type.is_block_device() {
        Entry::block_device(relative)
    } else {
        return Ok(None);
    };
    Ok(Some(special.with_mode(metadata.mode())))
}

/// Classify `files` with up to `threads` threads, in `files` order.
fn classify_files(files: &[PendingFile], threads: usize) -> anyhow::Result<Vec<Option<FileType>>> {
    // Below this, starting threads costs more than classifying serially.
    const PARALLEL_THRESHOLD: usize = 256;

    let threads = threads.min(files.len().div_ceil(PARALLEL_THRESHOLD));
    if threads <= 1 {
        return Ok(files
            .iter()
            .map(|file| classify_path(&file.host_path))
            .collect());
    }
    // Plain scoped threads, not a rayon pool: this runs inside a lazy
    // initializer that other Rules wait on. A rayon worker waiting on another
    // pool runs queued jobs of its own pool, which can be a Rule that waits
    // for this very initializer, and that deadlocks.
    const BATCH: usize = 64;
    let next = AtomicUsize::new(0);
    let mut classified = vec![None; files.len()];
    let batches = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let start = next.fetch_add(BATCH, Ordering::Relaxed);
                        if start >= files.len() {
                            return done;
                        }
                        let end = (start + BATCH).min(files.len());
                        done.push((
                            start,
                            files[start..end]
                                .iter()
                                .map(|file| classify_path(&file.host_path))
                                .collect::<Vec<_>>(),
                        ));
                    }
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|worker| {
                worker
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect::<Vec<_>>()
    });
    for (start, batch) in batches.into_iter().flatten() {
        classified[start..start + batch.len()].copy_from_slice(&batch);
    }
    Ok(classified)
}

#[allow(dead_code)]
fn digest_md5(files: &FileTree, entry: &Entry) -> Option<String> {
    entry_bytes(files, entry).map(|bytes| hex_digest(DigestAlgorithm::Md5, &bytes))
}

fn set_file_type(entry: &mut Entry, classified: Option<FileType>) {
    if let Entry::RegularFile { file_type, .. } | Entry::Hardlink { file_type, .. } = entry {
        *file_type = classified;
    }
}

fn classify_path(path: &Path) -> Option<FileType> {
    let file = fs::File::open(path).ok()?;
    let mut magic = [0u8; 4];
    let n = file.read_at(&mut magic, 0).ok()?;
    if !FileType::peek_classifiable(&magic[..n]) {
        return None;
    }
    if magic.starts_with(b"\x7fELF") {
        // The headers say all a Rule needs; no need to read the whole file.
        return FileType::from_elf_file(&file);
    }
    drop(file);
    let bytes = fs::read(path).ok()?;
    FileType::from_bytes(&bytes)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::Path;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn query_filters_by_kind_and_glob() -> anyhow::Result<()> {
        let tree = FileTree::from_entries(
            None,
            vec![
                Entry::regular_file("bin/app.exe", Some(FileType::Pe32), None),
                Entry::directory("bin"),
                Entry::symlink("bin/link", "app.exe"),
                Entry::hardlink("lib/foo.dll", Some(FileType::Pe64), None),
            ],
        );
        let hits: Vec<_> = tree
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
            .glob("**/*.{exe,dll}")?
            .into_iter()
            .map(|entry| entry.path().to_path_buf())
            .collect();
        assert_eq!(
            hits,
            vec![PathBuf::from("bin/app.exe"), PathBuf::from("lib/foo.dll")]
        );
        Ok(())
    }

    #[test]
    fn query_filters_by_path_regex() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let tree = FileTree::from_entries(
            None,
            vec![
                Entry::regular_file("usr/bin/app", None, None),
                Entry::directory("usr/bin"),
                Entry::hardlink("debian/example.substvars", None, None),
                Entry::regular_file(OsString::from_vec(vec![0xff, b'x']), None, None),
            ],
        );
        let under_usr_bin = Regex::new(r"^usr/bin/").expect("valid regex");
        let hits: Vec<_> = tree
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
            .path(&under_usr_bin)
            .into_iter()
            .map(|entry| entry.path().to_path_buf())
            .collect();
        assert_eq!(hits, vec![PathBuf::from("usr/bin/app")]);

        let any = Regex::new(".").expect("valid regex");
        let utf8: Vec<_> = tree
            .query()
            .path(&any)
            .into_iter()
            .map(|entry| entry.path().to_path_buf())
            .collect();
        assert_eq!(
            utf8,
            vec![
                PathBuf::from("debian/example.substvars"),
                PathBuf::from("usr/bin"),
                PathBuf::from("usr/bin/app"),
            ]
        );
    }

    /// A tree large enough to classify on several threads, with ELF files,
    /// built the same way from inside a rayon pool (as a lint run does) and
    /// from a single thread.
    #[test]
    fn parallel_classification_matches_serial() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        let elf = fs::read(std::env::current_exe()?)?;
        for index in 0..700 {
            let name = dir.path().join(format!("file-{index:04}"));
            match index % 3 {
                0 => fs::write(name, &elf[..elf.len().min(1 << 20)])?,
                1 => fs::write(name, b"MZ\0\0not pe")?,
                _ => fs::write(name, b"plain text")?,
            }
        }
        let summary = |tree: &FileTree| -> Vec<(PathBuf, Option<FileType>)> {
            tree.entries()
                .iter()
                .map(|entry| (entry.path().to_path_buf(), entry.file_type()))
                .collect()
        };

        let serial = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()?
            .install(|| FileTree::from_host_directory(dir.path()))?;
        let parallel = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()?
            .install(|| FileTree::from_host_directory(dir.path()))?;
        assert_eq!(summary(&parallel), summary(&serial));
        assert_eq!(
            parallel
                .entries()
                .iter()
                .filter(|entry| entry.file_type() == Some(FileType::DosMz))
                .count(),
            233
        );
        Ok(())
    }

    /// The walk lists everything: hidden files, VCS directories, and paths a
    /// `.gitignore` or `.ignore` names. Symlinks to directories are not followed.
    #[test]
    fn from_host_directory_does_not_filter_or_follow_links() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        fs::create_dir_all(dir.path().join(".git/objects"))?;
        fs::create_dir_all(dir.path().join("sub/deep"))?;
        fs::write(dir.path().join(".gitignore"), "ignored\n")?;
        fs::write(dir.path().join(".ignore"), "also-ignored\n")?;
        fs::write(dir.path().join("ignored"), b"x")?;
        fs::write(dir.path().join("also-ignored"), b"x")?;
        fs::write(dir.path().join(".hidden"), b"x")?;
        fs::write(dir.path().join(".git/objects/blob"), b"x")?;
        fs::write(dir.path().join("sub/deep/leaf"), b"x")?;
        symlink("sub", dir.path().join("sub-link"))?;

        let tree = FileTree::from_host_directory(dir.path())?;
        let paths: Vec<_> = tree
            .entries()
            .iter()
            .map(|entry| entry.path().to_str().expect("utf-8 path").to_string())
            .collect();
        assert_eq!(
            paths,
            [
                ".git",
                ".git/objects",
                ".git/objects/blob",
                ".gitignore",
                ".hidden",
                ".ignore",
                "also-ignored",
                "ignored",
                // Paths sort by component, so `sub/*` precedes `sub-link`.
                "sub",
                "sub/deep",
                "sub/deep/leaf",
                "sub-link",
            ]
        );
        assert_eq!(
            tree.get("sub-link").and_then(Entry::link_target),
            Some(Path::new("sub"))
        );
        Ok(())
    }

    #[test]
    fn from_host_directory_reports_unreadable_directories() -> anyhow::Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new()?;
        let locked = dir.path().join("locked");
        fs::create_dir(&locked)?;
        fs::write(locked.join("file"), b"x")?;
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000))?;
        // Root can read anything, so there is nothing to fail on.
        let readable_anyway = fs::read_dir(&locked).is_ok();
        let result = FileTree::from_host_directory(dir.path());
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755))?;
        if !readable_anyway {
            let Err(error) = result else {
                panic!("unreadable directory must fail the walk");
            };
            assert!(
                format!("{error:#}").contains("locked"),
                "error should name the directory: {error:#}"
            );
        }
        Ok(())
    }

    #[test]
    fn from_host_directory_keeps_dot_pc_and_classifies() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        fs::create_dir_all(dir.path().join(".pc"))?;
        fs::write(dir.path().join(".pc/foo.exe"), b"MZ\0\0not pe")?;
        fs::write(dir.path().join("readme"), b"hi")?;
        symlink("readme", dir.path().join("link"))?;
        let tree = FileTree::from_host_directory(dir.path())?;
        assert!(tree.entries().iter().any(|entry| {
            entry.path() == Path::new(".pc/foo.exe")
                && entry.kind() == EntryKind::RegularFile
                && entry.file_type() == Some(FileType::DosMz)
        }));
        assert!(tree.entries().iter().any(|entry| {
            entry.path() == Path::new("readme")
                && entry.kind() == EntryKind::RegularFile
                && entry.file_type().is_none()
        }));
        assert!(tree.entries().iter().any(|entry| {
            entry.path() == Path::new("link")
                && entry.kind() == EntryKind::Symlink
                && entry.file_type().is_none()
                && entry.link_target() == Some(Path::new("readme"))
        }));
        assert!(
            tree.get("readme")
                .is_some_and(|entry| entry.link_target().is_none())
        );
        Ok(())
    }

    #[test]
    fn from_host_directory_keeps_permission_bits() -> anyhow::Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new()?;
        let set = |name: &str, mode: u32| {
            fs::set_permissions(dir.path().join(name), fs::Permissions::from_mode(mode))
        };
        fs::write(dir.path().join("suid"), b"#!/bin/sh\n")?;
        set("suid", 0o4755)?;
        fs::write(dir.path().join("private"), b"#!/bin/sh\n")?;
        set("private", 0o750)?;
        fs::write(dir.path().join("data"), b"x")?;
        set("data", 0o640)?;
        fs::create_dir(dir.path().join("shared"))?;
        set("shared", 0o1777)?;
        symlink("data", dir.path().join("link"))?;

        let tree = FileTree::from_host_directory(dir.path())?;
        let mode = |name: &str| tree.get(name).map(Entry::mode);
        assert_eq!(mode("suid"), Some(0o4755));
        assert_eq!(mode("private"), Some(0o750));
        assert_eq!(mode("data"), Some(0o640));
        assert_eq!(mode("shared"), Some(0o1777));
        assert_eq!(mode("link"), Some(0o777));
        let executable = |name: &str| tree.get(name).is_some_and(Entry::is_executable_file);
        assert!(executable("suid"));
        assert!(executable("private"));
        assert!(!executable("data"));
        // Directories and symlinks are never executable files, whatever the bits.
        assert!(!executable("shared"));
        assert!(!executable("link"));
        Ok(())
    }

    #[test]
    fn entry_mode_defaults_by_kind_and_masks_type_bits() {
        assert_eq!(Entry::regular_file("f", None, None).mode(), 0o644);
        assert_eq!(Entry::hardlink("h", None, None).mode(), 0o644);
        assert_eq!(Entry::directory("d").mode(), 0o755);
        assert_eq!(Entry::symlink("s", "f").mode(), 0o777);
        // S_IFREG | 06755 from a header that keeps the type bits.
        let entry = Entry::regular_file("f", None, None).with_mode(0o106_755);
        assert_eq!(entry.mode(), 0o6755);
        assert!(entry.is_executable_file());
        assert!(!Entry::regular_file("f", None, None).is_executable_file());
        assert!(
            Entry::hardlink("h", None, None)
                .with_mode(0o701)
                .is_executable_file()
        );
    }

    #[test]
    fn owner_reads_names_then_ids_and_root_for_zero() {
        let named = Owner::named(0, 60, "root", "games");
        assert_eq!(named.identity(), "root/games");
        assert_eq!(
            (named.user().as_str(), named.group().as_str()),
            ("root", "games")
        );
        assert_eq!((named.uid(), named.gid()), (0, 60));
        // No names: the number, but 0 is root.
        assert_eq!(Owner::new(0, 0, None, None).identity(), "root/root");
        assert_eq!(Owner::new(1000, 50, None, None).identity(), "1000/50");
        // An empty name is no name.
        assert_eq!(
            Owner::new(7, 8, Some(String::new()), Some("staff".to_string())).identity(),
            "7/staff"
        );
        assert_eq!(Owner::root().identity(), "root/root");
    }

    #[test]
    fn mtime_is_unknown_until_set_and_dates_in_utc() {
        let entry = Entry::regular_file("usr/bin/foo", None, None);
        assert_eq!(entry.mtime(), None);
        assert_eq!(entry.mtime_date(), None);
        let epoch = entry.clone().with_mtime(Some(0));
        assert_eq!(epoch.mtime(), Some(0));
        assert_eq!(epoch.mtime_date().as_deref(), Some("1970-01-01"));
        let dated = Entry::directory("usr").with_mtime(Some(1_000_000_000));
        assert_eq!(dated.mtime_date().as_deref(), Some("2001-09-09"));
        assert_eq!(dated.with_mtime(None).mtime(), None);
    }

    #[test]
    fn entries_have_no_owner_until_one_is_set() {
        let entries = [
            Entry::regular_file("f", None, None),
            Entry::hardlink("h", None, None),
            Entry::directory("d"),
            Entry::symlink("s", "f"),
            Entry::fifo("p"),
            Entry::char_device("c"),
            Entry::block_device("b"),
        ];
        for entry in entries {
            assert!(entry.owner().is_none(), "{:?}", entry.path());
            assert!(!entry.has_identity("root/root"));
            let owned = entry.with_owner(Owner::named(0, 60, "root", "games"));
            assert!(owned.has_identity("root/games"));
            assert!(!owned.has_identity("root/root"));
            assert_eq!(owned.owner().map(Owner::gid), Some(60));
        }
    }

    #[test]
    fn special_entries_are_not_files_and_keep_mode() {
        let fifo = Entry::fifo("etc/init.d/pipe");
        assert_eq!(fifo.kind(), EntryKind::Fifo);
        assert_eq!(Entry::char_device("c").kind(), EntryKind::CharDevice);
        assert_eq!(Entry::block_device("b").kind(), EntryKind::BlockDevice);
        assert_eq!(fifo.mode(), 0o644);
        assert_eq!(fifo.clone().with_mode(0o10_600).mode(), 0o600);
        for entry in [
            fifo,
            Entry::char_device("c").with_mode(0o755),
            Entry::block_device("b").with_mode(0o4755),
        ] {
            assert!(!entry.is_file());
            assert!(!entry.is_regular_file());
            assert!(!entry.is_dir());
            assert!(!entry.is_symlink());
            assert!(!entry.is_hardlink());
            assert!(!entry.is_executable_file());
            assert!(entry.file_type().is_none());
            assert!(entry.link_target().is_none());
            assert!(entry.hardlink_target().is_none());
        }
        let tree = FileTree::from_entries(None, vec![Entry::fifo("pipe")]);
        assert!(tree.read(tree.get("pipe").expect("pipe")).is_err());
    }

    #[test]
    fn lintian_predicates_follow_the_variant() {
        let regular = Entry::regular_file("f", None, None);
        let hardlink = Entry::hardlink("h", None, None);
        assert!(regular.is_file() && regular.is_regular_file() && !regular.is_hardlink());
        assert!(hardlink.is_file() && !hardlink.is_regular_file() && hardlink.is_hardlink());
        assert!(Entry::directory("d").is_dir());
        assert!(Entry::symlink("s", "f").is_symlink());
        assert!(!Entry::symlink("s", "f").is_file());
    }

    /// Host File trees know no owner: the host's numeric ids are not a
    /// package's names. They list fifos, which a tar archive can hold.
    #[test]
    fn from_host_directory_has_no_owner_and_lists_fifos() -> anyhow::Result<()> {
        let dir = TempDir::new()?;
        fs::write(dir.path().join("file"), b"x")?;
        fs::create_dir(dir.path().join("sub"))?;
        symlink("file", dir.path().join("link"))?;
        let made = std::process::Command::new("mkfifo")
            .arg(dir.path().join("pipe"))
            .status()?;
        assert!(made.success());

        let tree = FileTree::from_host_directory(dir.path())?;
        assert!(tree.entries().iter().all(|entry| entry.owner().is_none()));
        assert_eq!(tree.get("pipe").map(Entry::kind), Some(EntryKind::Fifo));
        // Not a file: no read, and not listed as one.
        let pipe = tree.get("pipe").expect("pipe");
        assert!(tree.read(pipe).is_err());
        assert_eq!(
            tree.query()
                .kinds([EntryKind::Fifo])
                .into_iter()
                .map(|entry| entry.path().to_path_buf())
                .collect::<Vec<_>>(),
            vec![PathBuf::from("pipe")]
        );
        Ok(())
    }

    #[test]
    fn hardlink_target_is_only_on_hardlinks() {
        let hardlink = Entry::hardlink("usr/bin/b", None, None).with_hardlink_target("usr/bin/a");
        assert_eq!(hardlink.hardlink_target(), Some(Path::new("usr/bin/a")));
        assert!(hardlink.link_target().is_none());
        assert!(
            Entry::hardlink("usr/bin/b", None, None)
                .hardlink_target()
                .is_none()
        );
        let symlink = Entry::symlink("usr/bin/c", "a").with_hardlink_target("usr/bin/a");
        assert!(symlink.hardlink_target().is_none());
        assert_eq!(symlink.link_target(), Some(Path::new("a")));
    }

    #[test]
    fn read_uses_fixture_bytes() -> anyhow::Result<()> {
        let tree = FileTree::from_entries(
            None,
            vec![
                Entry::regular_file("debian/control", None, Some(b"Source: example\n".to_vec())),
                Entry::directory("debian"),
            ],
        );
        let entry = tree.get("debian/control").expect("debian/control");
        assert_eq!(tree.read(entry)?, b"Source: example\n");
        assert!(tree.get("debian/rules").is_none());
        let directory = tree.get("debian").expect("debian");
        assert!(tree.read(directory).is_err());
        Ok(())
    }
}

#[cfg(test)]
mod md5_tests {
    use super::*;
    use crate::lint::digest::{DigestAlgorithm, hex_digest};

    #[test]
    fn md5sum_of_entries_is_digest_of_their_bytes() {
        let tree = FileTree::from_entries(
            None,
            vec![
                Entry::regular_file("a.bin", None, Some(b"same".to_vec())),
                Entry::hardlink("b.bin", None, Some(b"same".to_vec())),
                Entry::directory("dir"),
                Entry::symlink("link", "a.bin"),
            ],
        );
        let expected = hex_digest(DigestAlgorithm::Md5, b"same");
        assert_eq!(
            tree.md5sum(tree.get("a.bin").expect("a")),
            Some(expected.clone())
        );
        assert_eq!(tree.md5sum(tree.get("b.bin").expect("b")), Some(expected));
        assert_eq!(tree.md5sum(tree.get("dir").expect("dir")), None);
        assert_eq!(tree.md5sum(tree.get("link").expect("link")), None);
    }

    #[test]
    fn md5sum_is_shared_by_repeated_asks() {
        let tree = FileTree::from_entries(
            None,
            vec![Entry::regular_file("a.bin", None, Some(b"x".to_vec()))],
        );
        let entry = tree.get("a.bin").expect("a");
        let first = tree.md5sum(entry);
        assert!(
            tree.md5sums[0].get().is_some(),
            "digest cached after first ask"
        );
        assert_eq!(tree.md5sum(entry), first);
    }

    #[test]
    fn md5sum_of_an_entry_outside_the_tree_is_computed_without_caching() {
        let tree = FileTree::from_entries(
            None,
            vec![Entry::regular_file("a.bin", None, Some(b"x".to_vec()))],
        );
        let foreign = Entry::regular_file("a.bin", None, Some(b"x".to_vec()));
        assert_eq!(
            tree.md5sum(&foreign),
            Some(hex_digest(DigestAlgorithm::Md5, b"x"))
        );
        assert!(tree.md5sums[0].get().is_none());
    }

    #[test]
    fn host_file_without_root_has_no_md5() {
        let tree = FileTree::from_entries(None, vec![Entry::regular_file("a.bin", None, None)]);
        assert_eq!(tree.md5sum(tree.get("a.bin").expect("a")), None);
    }
}
