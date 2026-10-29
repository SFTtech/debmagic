use std::fs;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

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

    pub fn from_host_directory(root: impl AsRef<Path>) -> anyhow::Result<Self> {
        let root = root.as_ref();
        let mut entries = Vec::new();
        collect_entries(root, root, &mut entries)
            .with_context(|| format!("walking File tree at {}", root.display()))?;
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

fn collect_entries(root: &Path, dir: &Path, entries: &mut Vec<Entry>) -> anyhow::Result<()> {
    let read = fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?;
    for child in read {
        let child = child.with_context(|| format!("reading {}", dir.display()))?;
        let path = child.path();
        let relative = path
            .strip_prefix(root)
            .with_context(|| format!("relativizing {}", path.display()))?
            .to_path_buf();
        let metadata =
            fs::symlink_metadata(&path).with_context(|| format!("stat {}", path.display()))?;
        let Some(entry) = host_entry(&path, relative, &metadata)? else {
            continue;
        };
        let is_directory = matches!(entry, Entry::Directory { .. });
        entries.push(entry);
        if is_directory {
            collect_entries(root, &path, entries)?;
        }
    }
    Ok(())
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
        let classified = classify_path(path);
        return Ok(Some(if metadata.nlink() > 1 {
            Entry::hardlink(relative, classified, None)
        } else {
            Entry::regular_file(relative, classified, None)
        }));
    }
    Ok(None)
}

fn classify_path(path: &Path) -> Option<FileType> {
    let mut file = fs::File::open(path).ok()?;
    let mut magic = [0u8; 4];
    let n = file.read(&mut magic).ok()?;
    if !FileType::peek_classifiable(&magic[..n]) {
        return None;
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
