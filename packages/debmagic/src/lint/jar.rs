//! Jar files: their member names, class-file major versions, and manifest.
//!
//! Lintian's `Lintian::Index::Java` lists every file whose name ends in
//! `.jar` and whose `file(1)` type is a zip archive through `Archive::Zip`
//! (see [`crate::lint::zip`]), and keeps the member names with the class
//! file major version of each `.class` member, the manifest's headers, and
//! the read error. This reads the same things from an Entry's bytes. It
//! keeps lintian's quirks where a Rule can see them; they are listed on
//! [`Jar::read`] and [`Manifest`].

use std::collections::BTreeMap;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use super::elf::entry_bytes;
use super::file_tree::{Entry, FileTree};
use super::zip::{ZipArchive, ZipMember};

const CLASS_MAGIC: u32 = 0xCAFE_BABE;
/// Bytes `Lintian::Index::Java` needs of a class file to read its version.
const CLASS_HEADER_LENGTH: usize = 8;

/// The main headers of a jar's `META-INF/MANIFEST.MF`, as lintian reads them.
///
/// Every `Name: value` line of the whole file counts, in every section; a
/// later header replaces an earlier one with the same name. A header's
/// continuation lines (which start with a space) are dropped, so a long
/// `Class-Path` keeps only its first line. Values keep trailing whitespace.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    headers: BTreeMap<String, String>,
}

impl Manifest {
    fn parse(contents: &[u8]) -> Self {
        static HEADER: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"^(\S+):\s*(.*)").expect("manifest header regex"));
        let text = String::from_utf8_lossy(contents);
        let mut headers = BTreeMap::new();
        for line in text.split('\n') {
            let line = line.replace('\r', "");
            if let Some(captures) = HEADER.captures(&line) {
                headers.insert(captures[1].to_string(), captures[2].to_string());
            }
        }
        Self { headers }
    }

    /// The value of `header`, exactly as spelled.
    pub fn get(&self, header: &str) -> Option<&str> {
        self.headers.get(header).map(String::as_str)
    }
}

/// What lintian knows of one jar.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Jar {
    files: BTreeMap<String, Option<u16>>,
    manifest: Option<Manifest>,
    error: Option<String>,
}

impl Jar {
    /// `bytes` read as a jar. `None` unless they start like a zip archive
    /// (`PK\x03\x04`, or `PK\x05\x06` for an empty one), as `file(1)` sees
    /// it; `name` is the Entry path, for `Archive::Zip`'s messages.
    ///
    /// Lintian's quirks, kept: directories are not listed; a `.class` member
    /// shorter than eight bytes is not listed either; a `.class` member that
    /// cannot be read ends the list there; the manifest is the last member
    /// named `META-INF/MANIFEST.MF` in any letter case (the `.` matches any
    /// character), and one that cannot be read is an empty manifest; the
    /// version of a class whose magic is not `CAFEBABE` is absent.
    pub fn read(bytes: &[u8], name: &str) -> Option<Self> {
        if !(bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06")) {
            return None;
        }
        static MANIFEST_NAME: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"(?i)^META-INF/MANIFEST.MF$").expect("manifest regex"));
        let zip = match ZipArchive::parse(bytes, name) {
            Ok(zip) => zip,
            Err(error) => {
                return Some(Self {
                    files: BTreeMap::new(),
                    manifest: None,
                    error: Some(error.message().to_string()),
                });
            }
        };
        let mut files = BTreeMap::new();
        let mut manifest_member: Option<&ZipMember> = None;
        for member in zip.members() {
            if member.is_directory() {
                continue;
            }
            let member_name = member.name();
            if MANIFEST_NAME.is_match(member_name) {
                manifest_member = Some(member);
            }
            let mut version = None;
            if member_name.ends_with(".class") {
                let Ok(contents) = zip.contents(member) else {
                    break;
                };
                if contents.len() < CLASS_HEADER_LENGTH {
                    continue;
                }
                let magic =
                    u32::from_be_bytes([contents[0], contents[1], contents[2], contents[3]]);
                if magic == CLASS_MAGIC {
                    version = Some(u16::from_be_bytes([contents[6], contents[7]]));
                }
            }
            files.insert(member_name.to_string(), version);
        }
        let manifest = manifest_member.map(|member| {
            zip.contents(member)
                .map(|contents| Manifest::parse(&contents))
                .unwrap_or_default()
        });
        Some(Self {
            files,
            manifest,
            error: None,
        })
    }

    /// What lintian's `languages/java` Check finds among the jar's class-like
    /// members (`.class`, `.clj`, `.cljc`, in any letter case), walked in
    /// name order: the Java source each one sits next to, the first class with
    /// a version outside the known range (which ends the walk), and the lowest
    /// known version.
    pub fn scan_classes(&self) -> ClassScan {
        static CLASS_LIKE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"(?i)\.(?:class|cljc?)$").expect("class regex"));
        static EXTENSION: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"\.[^.]+$").expect("extension regex"));
        let limits = BytecodeLimits::vendored();
        let mut scan = ClassScan::default();
        for (name, version) in &self.files {
            if !CLASS_LIKE.is_match(name) {
                continue;
            }
            let source = EXTENSION.replace(name, ".java").into_owned();
            if self.files.contains_key(&source) {
                scan.sources.push(source);
            }
            scan.has_classes = true;
            if name.ends_with(".clj") || name.ends_with(".cljc") {
                continue;
            }
            let Some(version) = *version else {
                continue;
            };
            if version < limits.lowest || version > limits.highest {
                scan.unknown_version = Some((name.clone(), version));
                break;
            }
            scan.lowest_version = Some(scan.lowest_version.map_or(version, |l| l.min(version)));
        }
        scan
    }

    /// Member names with the major version of each class file; `None` for a
    /// member that is not a class file with the class magic.
    #[allow(dead_code)] // seam API; Rules read `scan_classes`
    pub fn files(&self) -> &BTreeMap<String, Option<u16>> {
        &self.files
    }

    pub fn manifest(&self) -> Option<&Manifest> {
        self.manifest.as_ref()
    }

    /// `Archive::Zip`'s error, when the archive could not be read.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

/// See [`Jar::scan_classes`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClassScan {
    /// Each class-like member whose `.java` twin is in the jar: that twin.
    pub sources: Vec<String>,
    pub has_classes: bool,
    /// The class that stopped the walk, with its version.
    pub unknown_version: Option<(String, u16)>,
    /// The lowest version of the classes walked, when there is one.
    pub lowest_version: Option<u16>,
}

/// `data/java/constants`: the class-file major versions lintian knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BytecodeLimits {
    pub lowest: u16,
    /// The highest version Debian's default JVM targets; newer class files
    /// are `incompatible-java-bytecode-format`.
    pub default: u16,
    pub highest: u16,
}

impl BytecodeLimits {
    pub fn vendored() -> Self {
        let value = |key: &str| {
            super::data::lines(include_str!("data/java/constants"))
                .filter_map(super::data::assignment)
                .find_map(|(name, value)| (name == key).then(|| value.parse().ok()).flatten())
                .unwrap_or_else(|| panic!("vendored java/constants lacks {key}"))
        };
        Self {
            lowest: value("lowest-known-bytecode-version"),
            default: value("default-bytecode-version"),
            highest: value("highest-known-bytecode-version"),
        }
    }
}

/// A jar in a File tree.
#[derive(Debug, Clone)]
pub struct JarEntry {
    jar: Jar,
    executable: bool,
}

impl JarEntry {
    pub fn jar(&self) -> &Jar {
        &self.jar
    }

    /// The Entry is a regular file or hardlink with an execute bit.
    pub fn is_executable(&self) -> bool {
        self.executable
    }
}

/// The jars of a File tree, by path.
#[derive(Debug, Default)]
pub struct JarFiles {
    /// In byte order of the path, as lintian's `sorted_list`.
    jars: Vec<(PathBuf, JarEntry)>,
}

impl JarFiles {
    /// Every regular file and hardlink named `*.jar` (in any letter case)
    /// that starts like a zip archive. Entries that cannot be read are left
    /// out.
    pub fn from_tree(files: &FileTree) -> Self {
        static JAR_NAME: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"(?i)\S\.jar$").expect("jar name regex"));
        let mut jars: Vec<(PathBuf, JarEntry)> = files
            .query()
            .into_iter()
            .filter(|entry| entry.is_file())
            .filter(|entry| JAR_NAME.is_match(&entry.path().to_string_lossy()))
            .filter_map(|entry: &Entry| {
                let bytes = entry_bytes(files, entry)?;
                let jar = Jar::read(&bytes, &entry.path().to_string_lossy())?;
                Some((
                    entry.path().to_path_buf(),
                    JarEntry {
                        jar,
                        executable: entry.is_executable_file(),
                    },
                ))
            })
            .collect();
        jars.sort_by(|(a, _), (b, _)| a.as_os_str().as_bytes().cmp(b.as_os_str().as_bytes()));
        Self { jars }
    }

    /// In byte order of the path.
    pub fn iter(&self) -> impl Iterator<Item = (&Path, &JarEntry)> {
        self.jars.iter().map(|(path, jar)| (path.as_path(), jar))
    }

    /// The jars lintian's `languages/java` Check looks into: those that read
    /// without error, except inside a JVM (`usr/lib/jvm/<jvm>/`,
    /// `usr/lib/jvm-exports/<jvm>/`) or Mozilla's `usr/lib/xul-ext/` and
    /// `usr/lib/xulrunner*/`.
    pub fn inspected(&self) -> impl Iterator<Item = (&Path, &JarEntry)> {
        static SKIPPED: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"^usr/lib/(?:jvm(?:-exports)?/[^/]+/|xul(?:-ext|runner[^/]*)/)")
                .expect("skipped jar regex")
        });
        self.iter().filter(|(path, entry)| {
            entry.jar.error.is_none() && !SKIPPED.is_match(&path.to_string_lossy())
        })
    }
}

/// Builds small jars for tests.
#[cfg(test)]
pub(crate) struct TestJar {
    zip: super::zip::TestZip,
}

#[cfg(test)]
impl TestJar {
    pub fn new() -> Self {
        Self {
            zip: super::zip::TestZip::new(),
        }
    }

    pub fn manifest(self, text: &str) -> Self {
        Self {
            zip: self.zip.member("META-INF/MANIFEST.MF", text.as_bytes()),
        }
    }

    /// A class file of this major version.
    pub fn class(self, name: &str, major: u16) -> Self {
        let mut bytes = vec![0xca, 0xfe, 0xba, 0xbe, 0, 0];
        bytes.extend_from_slice(&major.to_be_bytes());
        Self {
            zip: self.zip.member(name, &bytes),
        }
    }

    pub fn file(self, name: &str, contents: &[u8]) -> Self {
        Self {
            zip: self.zip.member(name, contents),
        }
    }

    pub fn build(&self) -> Vec<u8> {
        self.zip.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_files_versions_and_manifest() {
        let bytes = TestJar::new()
            .manifest("Manifest-Version: 1.0\r\nMain-Class: org.Main\r\nClass-Path: a.jar\n b.jar\n\nName: x\nSealed: true\n")
            .class("org/Main.class", 52)
            .class("org/Old.class", 49)
            .file("org/Main.java", b"class Main {}")
            .file("README", b"text")
            .build();
        let jar = Jar::read(&bytes, "x.jar").expect("jar");
        assert_eq!(jar.error(), None);
        assert_eq!(
            jar.files()
                .iter()
                .map(|(n, v)| (n.as_str(), *v))
                .collect::<Vec<_>>(),
            [
                ("META-INF/MANIFEST.MF", None),
                ("README", None),
                ("org/Main.class", Some(52)),
                ("org/Main.java", None),
                ("org/Old.class", Some(49)),
            ]
        );
        let manifest = jar.manifest().expect("manifest");
        assert_eq!(manifest.get("Main-Class"), Some("org.Main"));
        // The continuation line is dropped.
        assert_eq!(manifest.get("Class-Path"), Some("a.jar"));
        // Headers of later sections are read too.
        assert_eq!(manifest.get("Sealed"), Some("true"));
        assert_eq!(manifest.get("Missing"), None);
    }

    #[test]
    fn class_members_without_a_version_and_too_short_ones() {
        let bytes = super::super::zip::TestZip::new()
            .member("a/NotJava.class", b"not a class file at all")
            .member("a/Tiny.class", b"short")
            .member("clj/core.clj", b"(ns core)")
            .build();
        let jar = Jar::read(&bytes, "x.jar").expect("jar");
        // `Tiny.class` is not listed, as in lintian.
        assert_eq!(
            jar.files().keys().map(String::as_str).collect::<Vec<_>>(),
            ["a/NotJava.class", "clj/core.clj"]
        );
        assert_eq!(jar.files()["a/NotJava.class"], None);
    }

    #[test]
    fn scan_pairs_sources_and_stops_at_an_unknown_version() {
        let bytes = TestJar::new()
            .class("a/A.class", 52)
            .file("a/A.java", b"class A {}")
            .class("b/B.class", 42)
            .class("c/C.class", 49)
            .file("c/C.java", b"class C {}")
            .build();
        let scan = Jar::read(&bytes, "x.jar").expect("jar").scan_classes();
        // `c/C.class` is after the stop: its source is not found.
        assert_eq!(scan.sources, ["a/A.java"]);
        assert!(scan.has_classes);
        assert_eq!(scan.unknown_version, Some(("b/B.class".to_string(), 42)));
        assert_eq!(scan.lowest_version, Some(52));
    }

    #[test]
    fn scan_counts_clojure_and_uppercase_class_members_without_versions() {
        let bytes = TestJar::new()
            .file("x/y.clj", b"(ns y)")
            .file("X.CLASS", b"data")
            .build();
        let scan = Jar::read(&bytes, "x.jar").expect("jar").scan_classes();
        assert!(scan.has_classes);
        assert_eq!(scan.unknown_version, None);
        assert_eq!(scan.lowest_version, None);
        let none = Jar::read(&TestJar::new().file("README", b"x").build(), "x.jar")
            .expect("jar")
            .scan_classes();
        assert!(!none.has_classes);
    }

    #[test]
    fn bytecode_limits_come_from_the_vendored_table() {
        let limits = BytecodeLimits::vendored();
        assert_eq!(limits.lowest, 45);
        assert!(limits.lowest < limits.default && limits.default <= limits.highest);
    }

    #[test]
    fn the_manifest_is_found_in_any_letter_case() {
        let bytes = super::super::zip::TestZip::new()
            .member("meta-inf/manifest.mf", b"Main-Class: A\n")
            .build();
        let jar = Jar::read(&bytes, "x.jar").expect("jar");
        assert_eq!(jar.manifest().and_then(|m| m.get("Main-Class")), Some("A"));
    }

    #[test]
    fn an_unreadable_archive_keeps_only_the_error() {
        let bytes = TestJar::new().class("a/B.class", 52).build();
        let jar = Jar::read(&bytes[..bytes.len() - 25], "x.jar").expect("jar");
        assert_eq!(jar.error(), Some("format error: can't find EOCD signature"));
        assert!(jar.files().is_empty());
    }

    #[test]
    fn only_zip_archives_are_jars() {
        assert_eq!(Jar::read(b"not a zip", "x.jar"), None);
        assert_eq!(Jar::read(b"", "x.jar"), None);
        let empty = super::super::zip::TestZip::new().build();
        assert!(Jar::read(&empty, "x.jar").is_some());
    }

    #[test]
    fn jar_files_walks_jars_by_name_and_skips_jvm_and_xul() {
        let jar = TestJar::new().class("a/B.class", 52).build();
        let tree = FileTree::from_entries(
            None,
            vec![
                Entry::regular_file("usr/share/java/a.jar", None, Some(jar.clone().into())),
                Entry::regular_file("usr/share/java/b.JAR", None, Some(jar.clone().into()))
                    .with_mode(0o755),
                Entry::hardlink("usr/share/java/c.jar", None, Some(jar.clone().into())),
                Entry::regular_file(
                    "usr/share/java/not.jar",
                    None,
                    Some(b"text".to_vec().into()),
                ),
                Entry::regular_file("usr/share/java/a.zip", None, Some(jar.clone().into())),
                Entry::regular_file(
                    "usr/lib/jvm/java-21/lib/j.jar",
                    None,
                    Some(jar.clone().into()),
                ),
                Entry::regular_file("usr/lib/xulrunner-1/x.jar", None, Some(jar.clone().into())),
                Entry::regular_file("usr/lib/jvm/direct.jar", None, Some(jar.into())),
            ],
        );
        let jars = JarFiles::from_tree(&tree);
        let all: Vec<_> = jars
            .iter()
            .map(|(p, _)| p.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            all,
            [
                "usr/lib/jvm/direct.jar",
                "usr/lib/jvm/java-21/lib/j.jar",
                "usr/lib/xulrunner-1/x.jar",
                "usr/share/java/a.jar",
                "usr/share/java/b.JAR",
                "usr/share/java/c.jar",
            ]
        );
        let inspected: Vec<_> = jars
            .inspected()
            .map(|(p, e)| (p.to_string_lossy().into_owned(), e.is_executable()))
            .collect();
        assert_eq!(
            inspected,
            [
                ("usr/lib/jvm/direct.jar".to_string(), false),
                ("usr/share/java/a.jar".to_string(), false),
                ("usr/share/java/b.JAR".to_string(), true),
                ("usr/share/java/c.jar".to_string(), false),
            ]
        );
    }
}
