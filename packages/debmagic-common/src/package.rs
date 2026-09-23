use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::debian::changelog::ChangelogHead;
use crate::debian::copyright::FilesExcluded;
use crate::debian::source::SourceFormat;
use crate::debian::version::PackageVersion;

/// Reads the raw content of a `debian/` metadata file, given its
/// location relative to the package root (e.g. `"changelog"`,
/// `"source/format"`, `"copyright"`). Implemented by the consumer,
/// so this crate never touches the filesystem.
pub type FileReader = Box<dyn Fn(&str) -> anyhow::Result<String>>;

/// Where a package's files come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    /// A source tree on disk; `debian/` lives inside this dir.
    SourceDir(PathBuf),
    /// In-memory contents, e.g. a test fixture built with
    /// [`SourcePackage::from_files`].
    InMemory,
}

/// A Debian source package, as modeled from its `debian/` metadata.
///
/// The aggregate of the parsed substructures; the entry point for
/// anything that asks "what is this package". Substructures are read
/// lazily through the [`FileReader`] the consumer supplies at
/// construction, so I/O stays outside this crate and only the
/// metadata actually requested is ever read.
#[derive(Clone)]
pub struct SourcePackage {
    reader: Rc<FileReader>,
    location: Location,
    changelog: ChangelogHead,
    source_format: SourceFormat,
}

impl std::fmt::Debug for SourcePackage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourcePackage")
            .field("location", &self.location)
            .field("changelog", &self.changelog)
            .field("source_format", &self.source_format)
            .finish()
    }
}

impl SourcePackage {
    /// Build the model from a reader of the package's `debian/`
    /// metadata files and where those files live. Reads `changelog`
    /// and `source/format` eagerly (every consumer needs them);
    /// everything else stays lazy.
    pub fn from_reader(reader: Rc<FileReader>, location: Location) -> anyhow::Result<Self> {
        let changelog_content = reader("changelog")?;
        let changelog = changelog_content
            .parse::<debian_changelog::ChangeLog>()
            .map_err(|error| anyhow::anyhow!("failed to parse changelog: {error}"))?;
        let changelog = ChangelogHead::from_changelog(&changelog).ok_or_else(|| {
            anyhow::anyhow!("changelog head entry has no package, version or distribution")
        })?;
        let source_format = reader("source/format")
            .map(|content| SourceFormat::parse(&content))
            .unwrap_or_default();
        Ok(Self {
            reader,
            location,
            changelog,
            source_format,
        })
    }

    /// Build the model from in-memory file contents, keyed by their
    /// `debian/`-relative name (`"changelog"`, `"source/format"`, ...).
    /// The file-less construction for tests and other synthetic
    /// packages; a file absent from the map behaves like one missing
    /// on disk.
    pub fn from_files<I, S>(files: I) -> anyhow::Result<Self>
    where
        I: IntoIterator<Item = (S, S)>,
        S: Into<String>,
    {
        let files: std::collections::HashMap<String, String> = files
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        let reader: FileReader = Box::new(move |name: &str| {
            files
                .get(name)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("no such file: {name}"))
        });
        SourcePackage::from_reader(Rc::new(reader), Location::InMemory)
    }

    /// The source package name.
    pub fn name(&self) -> &str {
        &self.changelog.package
    }

    /// The version of the latest changelog entry.
    pub fn version(&self) -> &PackageVersion {
        &self.changelog.version
    }

    /// The raw distribution names of the latest changelog entry (not resolved
    /// to a [`crate::distro::DistroVersion`]).
    pub fn distributions(&self) -> &[String] {
        &self.changelog.distributions
    }

    /// A native package has no `orig` tarball at all.
    pub fn is_native(&self) -> bool {
        self.source_format.is_native()
    }

    /// Where the package's files live; the source tree dir for a
    /// package opened from disk.
    pub fn location(&self) -> &Location {
        &self.location
    }

    /// The source tree dir, for a package opened from disk.
    pub fn source_dir(&self) -> anyhow::Result<&Path> {
        match &self.location {
            Location::SourceDir(dir) => Ok(dir),
            Location::InMemory => Err(anyhow::anyhow!(
                "this package has no source dir; it was built from in-memory files"
            )),
        }
    }

    /// The `Files-Excluded` patterns from `debian/copyright`.
    /// Read lazily; a missing file yields no excludes.
    pub fn copyright_excludes(&self) -> anyhow::Result<FilesExcluded> {
        Ok((self.reader)("copyright")
            .map(|content| crate::debian::copyright::files_excluded(&content))
            .unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// A reader over an in-memory file map that records which files
    /// were read, so tests can assert laziness.
    fn reader(files: &[(&str, &str)]) -> (Rc<FileReader>, Rc<RefCell<Vec<String>>>) {
        let files: HashMap<String, String> = files
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let reads = Rc::new(RefCell::new(Vec::new()));
        let reads_clone = reads.clone();
        let reader: FileReader = Box::new(move |name: &str| {
            reads_clone.borrow_mut().push(name.to_string());
            files
                .get(name)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("no such file: {name}"))
        });
        (Rc::new(reader), reads)
    }

    const CHANGELOG: &str = "postfix (3.11.7-1) unstable; urgency=medium\n\n  * Change.\n\n -- A <a@example.com>  Mon, 01 Jan 2024 00:00:00 +0000\n";

    #[test]
    fn test_accessors() {
        let (reader, reads) =
            reader(&[("changelog", CHANGELOG), ("source/format", "3.0 (quilt)\n")]);
        let package = SourcePackage::from_reader(reader, Location::InMemory).unwrap();
        assert_eq!(package.name(), "postfix");
        assert_eq!(package.version().version(), "3.11.7-1");
        assert_eq!(package.distributions(), ["unstable"]);
        assert!(!package.is_native());
        // construction reads only the eager files
        assert_eq!(*reads.borrow(), ["changelog", "source/format"]);
    }

    #[test]
    fn test_native() {
        let (reader, _) = reader(&[
            ("changelog", CHANGELOG),
            ("source/format", "3.0 (native)\n"),
        ]);
        assert!(
            SourcePackage::from_reader(reader, Location::InMemory)
                .unwrap()
                .is_native()
        );
    }

    #[test]
    fn test_copyright_excludes_lazy() {
        let (reader, reads) = reader(&[
            ("changelog", CHANGELOG),
            ("source/format", "3.0 (quilt)\n"),
            ("copyright", "Files-Excluded:\n win32/*\n"),
        ]);
        let package = SourcePackage::from_reader(reader, Location::InMemory).unwrap();
        assert_eq!(*reads.borrow(), ["changelog", "source/format"]);
        let excludes = package.copyright_excludes().unwrap();
        assert_eq!(excludes.main, ["win32/*"]);
        assert_eq!(*reads.borrow(), ["changelog", "source/format", "copyright"]);
    }

    #[test]
    fn test_copyright_excludes_missing_file() {
        let (reader, _) = reader(&[("changelog", CHANGELOG), ("source/format", "3.0 (quilt)\n")]);
        let package = SourcePackage::from_reader(reader, Location::InMemory).unwrap();
        // a missing copyright is no error, just no excludes
        assert!(package.copyright_excludes().unwrap().main.is_empty());
    }

    #[test]
    fn test_from_files() {
        let package = SourcePackage::from_files([
            ("changelog", CHANGELOG),
            ("source/format", "3.0 (quilt)\n"),
            ("copyright", "Files-Excluded:\n win32/*\n"),
        ])
        .unwrap();
        assert_eq!(package.name(), "postfix");
        assert!(!package.is_native());
        assert_eq!(package.copyright_excludes().unwrap().main, ["win32/*"]);
    }

    #[test]
    fn test_from_files_missing_lazy_file() {
        // a file absent from the map behaves like one missing on disk
        let package = SourcePackage::from_files([
            ("changelog", CHANGELOG),
            ("source/format", "3.0 (quilt)\n"),
        ])
        .unwrap();
        assert!(package.copyright_excludes().unwrap().main.is_empty());
    }

    #[test]
    fn test_from_files_missing_changelog() {
        assert!(SourcePackage::from_files([("source/format", "3.0 (quilt)\n")]).is_err());
    }

    #[test]
    fn test_source_dir() {
        let (reader, _) = reader(&[("changelog", CHANGELOG), ("source/format", "3.0 (quilt)\n")]);
        let package =
            SourcePackage::from_reader(reader, Location::SourceDir(PathBuf::from("/tmp/pkg")))
                .unwrap();
        assert_eq!(package.source_dir().unwrap(), Path::new("/tmp/pkg"));
        assert_eq!(
            package.location(),
            &Location::SourceDir(PathBuf::from("/tmp/pkg"))
        );

        // an in-memory package has no source dir
        let package = SourcePackage::from_files([("changelog", CHANGELOG)]).unwrap();
        assert!(package.source_dir().is_err());
        assert_eq!(package.location(), &Location::InMemory);
    }
}
