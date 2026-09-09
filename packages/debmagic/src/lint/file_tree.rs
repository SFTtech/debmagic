use std::fs;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use globset::{Glob, GlobMatcher};

use super::file_type::FileType;

/// Whether an Entry is a regular file, directory, symlink, or hardlink.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryKind {
    RegularFile,
    Directory,
    Symlink,
    Hardlink,
}

/// A path in a File tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    path: PathBuf,
    kind: EntryKind,
    file_type: Option<FileType>,
    content: Option<Vec<u8>>,
}

impl Entry {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn kind(&self) -> EntryKind {
        self.kind
    }

    pub fn file_type(&self) -> Option<FileType> {
        self.file_type
    }

    pub(crate) fn with_content(
        path: impl Into<PathBuf>,
        kind: EntryKind,
        file_type: Option<FileType>,
        content: Vec<u8>,
    ) -> Self {
        Self {
            path: path.into(),
            kind,
            file_type,
            content: Some(content),
        }
    }

    #[cfg(test)]
    pub(crate) fn fixture(
        path: impl Into<PathBuf>,
        kind: EntryKind,
        file_type: Option<FileType>,
        content: Option<Vec<u8>>,
    ) -> Self {
        Self {
            path: path.into(),
            kind,
            file_type,
            content,
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
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        Self { root, entries }
    }

    pub fn from_host_directory(root: impl AsRef<Path>) -> anyhow::Result<Self> {
        let root = root.as_ref();
        let mut entries = Vec::new();
        collect_entries(root, root, &mut entries)
            .with_context(|| format!("walking File tree at {}", root.display()))?;
        apply_file_types(root, &mut entries);
        Ok(Self::from_entries(Some(root.to_path_buf()), entries))
    }

    #[allow(dead_code)]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn get(&self, path: impl AsRef<Path>) -> Option<&Entry> {
        let path = path.as_ref();
        self.entries.iter().find(|entry| entry.path == path)
    }

    pub fn query(&self) -> Query<'_> {
        Query {
            tree: self,
            kinds: None,
            glob: None,
        }
    }

    pub fn read(&self, entry: &Entry) -> anyhow::Result<Vec<u8>> {
        if let Some(bytes) = &entry.content {
            return Ok(bytes.clone());
        }
        let Some(root) = &self.root else {
            bail!(
                "Entry {} has no content and File tree has no directory root",
                entry.path.display()
            );
        };
        fs::read(root.join(&entry.path))
            .with_context(|| format!("reading {}", entry.path.display()))
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
            && !kinds.contains(&entry.kind)
        {
            return false;
        }
        if let Some(glob) = &self.glob
            && !glob.is_match(&entry.path)
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
        let Some(kind) = entry_kind(&metadata) else {
            continue;
        };
        entries.push(Entry {
            path: relative,
            kind,
            file_type: None,
            content: None,
        });
        if kind == EntryKind::Directory {
            collect_entries(root, &path, entries)?;
        }
    }
    Ok(())
}

fn entry_kind(metadata: &fs::Metadata) -> Option<EntryKind> {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Some(EntryKind::Symlink);
    }
    if file_type.is_dir() {
        return Some(EntryKind::Directory);
    }
    if file_type.is_file() {
        if metadata.nlink() > 1 {
            return Some(EntryKind::Hardlink);
        }
        return Some(EntryKind::RegularFile);
    }
    None
}

fn apply_file_types(root: &Path, entries: &mut [Entry]) {
    for entry in entries {
        if matches!(entry.kind, EntryKind::RegularFile | EntryKind::Hardlink) {
            entry.file_type = classify_path(&root.join(&entry.path));
        }
    }
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
                Entry::fixture(
                    "bin/app.exe",
                    EntryKind::RegularFile,
                    Some(FileType::Pe32),
                    None,
                ),
                Entry::fixture("bin", EntryKind::Directory, None, None),
                Entry::fixture("bin/link", EntryKind::Symlink, None, None),
                Entry::fixture(
                    "lib/foo.dll",
                    EntryKind::Hardlink,
                    Some(FileType::Pe64),
                    None,
                ),
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
        }));
        Ok(())
    }

    #[test]
    fn read_uses_fixture_bytes() -> anyhow::Result<()> {
        let tree = FileTree::from_entries(
            None,
            vec![Entry::fixture(
                "debian/control",
                EntryKind::RegularFile,
                None,
                Some(b"Source: example\n".to_vec()),
            )],
        );
        let entry = tree.get("debian/control").expect("debian/control");
        assert_eq!(tree.read(entry)?, b"Source: example\n");
        assert!(tree.get("debian/rules").is_none());
        Ok(())
    }
}
