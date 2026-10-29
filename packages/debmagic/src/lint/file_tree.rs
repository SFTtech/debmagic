use std::fs;
use std::os::unix::fs::{FileExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use anyhow::{Context, bail};
use globset::{Glob, GlobMatcher};

use super::file_type::FileType;

/// Which variant an [`Entry`] is, without its data. For filtering and matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryKind {
    RegularFile,
    Directory,
    Symlink,
    Hardlink,
}

/// A path in a File tree, with the data that variant has.
///
/// File `content` is `None` when the bytes live on the host and are read on
/// demand through [`FileTree::read`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    RegularFile {
        path: PathBuf,
        file_type: Option<FileType>,
        content: Option<Vec<u8>>,
    },
    Hardlink {
        path: PathBuf,
        file_type: Option<FileType>,
        content: Option<Vec<u8>>,
    },
    Directory {
        path: PathBuf,
    },
    /// `target` is exactly as stored, not resolved.
    Symlink {
        path: PathBuf,
        target: PathBuf,
    },
}

impl Entry {
    pub(crate) fn regular_file(
        path: impl Into<PathBuf>,
        file_type: Option<FileType>,
        content: Option<Vec<u8>>,
    ) -> Self {
        Self::RegularFile {
            path: path.into(),
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
            file_type,
            content,
        }
    }

    pub(crate) fn directory(path: impl Into<PathBuf>) -> Self {
        Self::Directory { path: path.into() }
    }

    pub(crate) fn symlink(path: impl Into<PathBuf>, target: impl Into<PathBuf>) -> Self {
        Self::Symlink {
            path: path.into(),
            target: target.into(),
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            Self::RegularFile { path, .. }
            | Self::Hardlink { path, .. }
            | Self::Directory { path }
            | Self::Symlink { path, .. } => path,
        }
    }

    pub fn kind(&self) -> EntryKind {
        match self {
            Self::RegularFile { .. } => EntryKind::RegularFile,
            Self::Hardlink { .. } => EntryKind::Hardlink,
            Self::Directory { .. } => EntryKind::Directory,
            Self::Symlink { .. } => EntryKind::Symlink,
        }
    }

    /// File type of a regular file or hardlink, when classifiable.
    pub fn file_type(&self) -> Option<FileType> {
        match self {
            Self::RegularFile { file_type, .. } | Self::Hardlink { file_type, .. } => *file_type,
            Self::Directory { .. } | Self::Symlink { .. } => None,
        }
    }

    /// Where a symlink points. `None` for every other variant.
    pub fn link_target(&self) -> Option<&Path> {
        match self {
            Self::Symlink { target, .. } => Some(target),
            _ => None,
        }
    }
}

/// Entries a Rule can walk as if they sat on disk.
pub struct FileTree {
    #[allow(dead_code)]
    root: Option<PathBuf>,
    entries: Vec<Entry>,
}

impl FileTree {
    pub fn from_entries(root: Option<PathBuf>, mut entries: Vec<Entry>) -> Self {
        entries.sort_by(|left, right| left.path().cmp(right.path()));
        Self { root, entries }
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

    pub fn get(&self, path: impl AsRef<Path>) -> Option<&Entry> {
        let path = path.as_ref();
        self.entries.iter().find(|entry| entry.path() == path)
    }

    pub fn query(&self) -> Query<'_> {
        Query {
            tree: self,
            kinds: None,
            glob: None,
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

/// Filter over a File tree: Entry kind and path glob.
pub struct Query<'a> {
    tree: &'a FileTree,
    kinds: Option<Vec<EntryKind>>,
    glob: Option<GlobMatcher>,
}

impl<'a> Query<'a> {
    pub fn kinds(mut self, kinds: impl IntoIterator<Item = EntryKind>) -> Self {
        self.kinds = Some(kinds.into_iter().collect());
        self
    }

    #[allow(dead_code)]
    pub fn glob(mut self, pattern: &str) -> anyhow::Result<Self> {
        self.glob = Some(Glob::new(pattern)?.compile_matcher());
        Ok(self)
    }
}

impl<'a> IntoIterator for Query<'a> {
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

impl Query<'_> {
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
        true
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
        return Ok(Some(Entry::symlink(relative, target)));
    }
    if file_type.is_dir() {
        return Ok(Some(Entry::directory(relative)));
    }
    if file_type.is_file() {
        return Ok(Some(if metadata.nlink() > 1 {
            Entry::hardlink(relative, None, None)
        } else {
            Entry::regular_file(relative, None, None)
        }));
    }
    Ok(None)
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
