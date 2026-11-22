use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::group_context::GroupContext;
use crate::lint::jar::JarFiles;
use crate::lint::rule::GroupRule;

pub struct MissingNoticeFileForApacheLicense;

declare_rule! {
    /// The copyright file names the Apache 2.0 license, the source has a
    /// `NOTICE` (or `NOTICE.txt`) file that mentions Apache, and no installable
    /// ships a file by that name. The Location is each such notice file of the
    /// source.
    ///
    /// The copyright files looked at are `debian/copyright` and
    /// `debian/<installable>.copyright` for each installable in
    /// `debian/control` that are valid UTF-8 and contain `apache`, space or
    /// `-`, and `2.` (any letter case). A notice file is a file of the
    /// Patched files, anywhere in the tree (`.pc/` too), whose name is
    /// `NOTICE` or `NOTICE.txt`, that can be read, and whose bytes contain
    /// `apache` in any letter case. An installable ships a notice when a path
    /// in it, or a path inside one of its jars, ends in `/NOTICE`,
    /// `/NOTICE.txt` or either with `.gz` (so a top-level `NOTICE` does not
    /// count, as in lintian). udebs are not looked at; a Group with no other
    /// installable is not reported.
    ///
    /// A Group Rule: it needs the installables' files, so it runs once per
    /// Group that has a `.dsc`. Where lintian reads a jar by its
    /// `java_info`, this Rule reads each `*.jar` that starts like a zip
    /// archive.
    MissingNoticeFileForApacheLicense,
    code = "LN1502",
    tag = "missing-notice-file-for-apache-license",
    default_selected = true,
    experimental = false,
}

static APACHE_2: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)apache[-\s]+2\.").expect("apache regex"));
static NOTICE_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^NOTICE(?:\.txt)?$").expect("notice regex"));
static NOTICE_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/NOTICE(?:\.txt)?(?:\.gz)?$").expect("notice path regex"));

impl GroupRule for MissingNoticeFileForApacheLicense {
    fn run(&self, ctx: &mut GroupContext<'_>) {
        let Some(control) = ctx.debian_control().cloned() else {
            return;
        };
        let Some(patched) = ctx.patched_files() else {
            return;
        };
        let candidates = std::iter::once("debian/copyright".to_string()).chain(
            control
                .installables()
                .iter()
                .map(|installable| format!("debian/{}.copyright", installable.name())),
        );
        let mentions_apache: Vec<String> = candidates
            .filter(|path| {
                patched
                    .get(path)
                    .and_then(|entry| open_bytes(patched, entry))
                    .and_then(|bytes| String::from_utf8(bytes).ok())
                    .is_some_and(|text| APACHE_2.is_match(&text))
            })
            .collect();
        if mentions_apache.is_empty() {
            return;
        }

        let notices: Vec<PathBuf> = patched
            .entries()
            .iter()
            .filter(|entry| {
                entry
                    .path()
                    .file_name()
                    .is_some_and(|name| NOTICE_NAME.is_match(&name.to_string_lossy()))
            })
            .filter(|entry| {
                open_bytes(patched, entry).is_some_and(|bytes| {
                    bytes
                        .to_ascii_lowercase()
                        .windows(6)
                        .any(|w| w == b"apache")
                })
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        if notices.is_empty() {
            return;
        }

        let installables = ctx.installables();
        let binaries: Vec<_> = installables.iter().filter(|i| !i.is_udeb()).collect();
        if binaries.is_empty() {
            return;
        }
        for binary in binaries {
            let Some(data) = ctx.data_files(binary) else {
                continue;
            };
            let in_paths = data
                .entries()
                .iter()
                .any(|entry| NOTICE_PATH.is_match(&entry.path().to_string_lossy()));
            if in_paths
                || JarFiles::from_tree(data).iter().any(|(_, jar)| {
                    jar.jar()
                        .files()
                        .keys()
                        .any(|name| NOTICE_PATH.is_match(name))
                })
            {
                return;
            }
        }

        // One finding for each copyright file that names the license.
        for _ in &mentions_apache {
            for notice in &notices {
                ctx.diagnostic(Diagnostic::error("").with_location(Location {
                    path: notice.clone(),
                    line: None,
                    column: None,
                }));
            }
        }
    }
}

/// The bytes of a file, or of the file a symlink leads to inside the tree
/// (lintian's `is_open_ok` and `bytes`); `None` for anything else.
fn open_bytes(tree: &FileTree, entry: &Entry) -> Option<Vec<u8>> {
    let mut entry = entry;
    for _ in 0..8 {
        match entry.kind() {
            EntryKind::RegularFile | EntryKind::Hardlink => return tree.read(entry).ok(),
            EntryKind::Symlink => {
                let target = entry.link_target()?;
                let mut resolved = entry.path().parent().unwrap_or(Path::new("")).to_path_buf();
                if target.is_absolute() {
                    return None;
                }
                for component in target.components() {
                    match component {
                        Component::ParentDir => {
                            if !resolved.pop() {
                                return None;
                            }
                        }
                        Component::Normal(name) => resolved.push(name),
                        _ => {}
                    }
                }
                entry = tree.get(&resolved)?;
            }
            _ => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use crate::lint::jar::TestJar;
    use crate::lint::tester::{GroupCase, GroupTester, TestFile};

    use super::*;

    const DSC: &str = "Source: example\nVersion: 1.0\n";
    const CONTROL: &str = "Source: example\n\nPackage: example\nArchitecture: all\nDescription: x\n x\n\nPackage: other\nArchitecture: all\nDescription: x\n x\n";
    const APACHE: &str = "Format: x\n\nFiles: *\nLicense: Apache-2.0\n";
    const NOTICE: &str =
        "This product includes software developed at the Apache Software Foundation\n";

    fn source(files: Vec<(&'static str, TestFile)>) -> GroupCase {
        let mut all = vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/copyright", TestFile::new(APACHE)),
        ];
        all.extend(files);
        GroupCase::new(DSC).source_files(all)
    }

    #[test]
    fn group() {
        let notice = || ("NOTICE", TestFile::new(NOTICE));
        let pass = vec![
            // No installable at all, or only a udeb.
            source(vec![notice()]),
            source(vec![notice()]).udeb("example-udeb", ""),
            // No NOTICE file in the source, or one without Apache.
            source(vec![]).installable("example", ""),
            source(vec![("NOTICE", TestFile::new("nothing\n"))]).installable("example", ""),
            // A copyright that does not name the license.
            GroupCase::new(DSC)
                .source_files([
                    ("debian/control", TestFile::new(CONTROL)),
                    ("debian/copyright", TestFile::new("License: GPL-2+\n")),
                    ("NOTICE", TestFile::new(NOTICE)),
                ])
                .installable("example", ""),
            // The copyright is not UTF-8.
            GroupCase::new(DSC)
                .source_files([
                    ("debian/control", TestFile::new(CONTROL)),
                    (
                        "debian/copyright",
                        TestFile::from_bytes(b"License: Apache-2.0 \xe9\n".to_vec()),
                    ),
                    ("NOTICE", TestFile::new(NOTICE)),
                ])
                .installable("example", ""),
            // A notice in any installable, in a path or in a jar.
            source(vec![notice()])
                .installable("example", "")
                .installable("other", "")
                .files([("usr/share/doc/other/NOTICE", TestFile::new(NOTICE))]),
            source(vec![notice()])
                .installable("example", "")
                .files([("usr/share/doc/example/NOTICE.txt.gz", TestFile::new(""))]),
            source(vec![notice()]).installable("example", "").files([(
                "usr/share/java/x.jar",
                TestFile::from_bytes(TestJar::new().file("META-INF/NOTICE", b"x").build()),
            )]),
            // The copyright of an installable names the license.
            GroupCase::new(DSC)
                .source_files([
                    ("debian/control", TestFile::new(CONTROL)),
                    ("debian/copyright", TestFile::new("License: GPL-2+\n")),
                    ("debian/other.copyright", TestFile::new(APACHE)),
                    ("NOTICE", TestFile::new(NOTICE)),
                ])
                .installable("example", "")
                .files([("usr/share/doc/example/NOTICE", TestFile::new(NOTICE))]),
        ];
        let fail = vec![
            source(vec![notice()]).installable("example", ""),
            // Only a top-level NOTICE is shipped: not counted.
            source(vec![notice()])
                .installable("example", "")
                .files([("NOTICE", TestFile::new(NOTICE))]),
            // Notice files anywhere, case-insensitive Apache; a name that
            // only looks similar is ignored.
            source(vec![
                ("NOTICE.txt", TestFile::new("APACHE\n")),
                ("subdir/NOTICE", TestFile::new(NOTICE)),
                ("subdir/NOTICE.md", TestFile::new(NOTICE)),
            ])
            .installable("example", ""),
            // A jar without a notice.
            source(vec![notice()]).installable("example", "").files([(
                "usr/share/java/x.jar",
                TestFile::from_bytes(TestJar::new().file("META-INF/MANIFEST.MF", b"x").build()),
            )]),
            // A symlinked notice file counts for the source.
            source(vec![
                ("docs/real", TestFile::new(NOTICE)),
                ("sub/NOTICE", TestFile::symlink("../docs/real")),
            ])
            .installable("example", ""),
            // Two copyright files that name the license: a finding for each.
            GroupCase::new(DSC)
                .source_files([
                    ("debian/control", TestFile::new(CONTROL)),
                    ("debian/copyright", TestFile::new(APACHE)),
                    ("debian/example.copyright", TestFile::new(APACHE)),
                    ("NOTICE", TestFile::new(NOTICE)),
                ])
                .installable("example", ""),
        ];
        GroupTester::new(MissingNoticeFileForApacheLicense, pass, fail).test_and_snapshot();
    }
}
