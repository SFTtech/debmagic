use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::external_tool;
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::interpreter::Hashbang;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

pub struct NationalEncoding;

declare_rule! {
    /// A packaging or Binary-package file is not valid UTF-8.
    ///
    /// On the Source tree and Source package, walks regular files and hardlinks
    /// under `debian/`. `file(1)` (`--raw --brief`, `LC_ALL=C`) must describe the
    /// file as text: the description ends with `text`. Files under
    /// `debian/patches/` are judged on the DEP-3 header only (bytes before the
    /// first line that starts with `---`); an empty file is skipped. Extra is
    /// `DEP-3 header` for a patch and empty otherwise.
    ///
    /// On the Binary package, including udebs, walks Control files that are
    /// text or a script (`#!`, but not the lincity `#!#!#!` magic) and Data
    /// files that are text. Gzip members under `usr/share/man/` are
    /// uncompressed and judged the same way. Extra is empty.
    ///
    /// Files already classified as ELF, PE, or DOS MZ are skipped. Location is
    /// the path, with no line number.
    NationalEncoding,
    code = "LN0018",
    tag = "national-encoding",
    default_selected = true,
    experimental = false,
}

impl SourceRule for NationalEncoding {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let hits = inspect(files, Place::Debian);
        for hit in hits {
            ctx.diagnostic(hit.into_diagnostic());
        }
    }
}

impl BinaryPackageRule for NationalEncoding {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let control_hits = ctx
            .control_files()
            .map(|files| inspect(files, Place::Control))
            .unwrap_or_default();
        let data_hits = ctx
            .data_files()
            .map(|files| inspect(files, Place::Installed))
            .unwrap_or_default();
        for hit in control_hits.into_iter().chain(data_hits) {
            ctx.diagnostic(hit.into_diagnostic());
        }
    }
}

#[derive(Clone, Copy)]
enum Place {
    Debian,
    Control,
    Installed,
}

struct Hit {
    message: &'static str,
    path: PathBuf,
}

impl Hit {
    fn into_diagnostic(self) -> Diagnostic {
        Diagnostic::warn(self.message).with_location(Location {
            path: self.path,
            line: None,
            column: None,
        })
    }
}

/// A candidate whose verdict is known, or one that needs `file`'s description
/// of its bytes before it can be judged.
enum Slot {
    Settled(Option<Hit>),
    Pending { path: PathBuf, bytes: Vec<u8> },
}

fn inspect(files: &FileTree, place: Place) -> Vec<Hit> {
    let mut slots = Vec::new();
    for entry in files.entries() {
        if !is_content_file(entry) || entry.file_type().is_some() {
            continue;
        }
        let path = entry.path();
        if matches!(place, Place::Debian) && !under(path, &["debian"]) {
            continue;
        }
        let Ok(bytes) = files.read(entry) else {
            continue;
        };
        if needs_description(&bytes, path, place) {
            slots.push(Slot::Pending {
                path: path.to_path_buf(),
                bytes,
            });
        } else {
            slots.push(Slot::Settled(hit_for(&bytes, path, place, None)));
        }
    }

    let descriptions = {
        let buffers: Vec<&[u8]> = slots
            .iter()
            .filter_map(|slot| match slot {
                Slot::Pending { bytes, .. } => Some(bytes.as_slice()),
                Slot::Settled(_) => None,
            })
            .collect();
        file_descriptions(&buffers)
    };
    let mut descriptions = descriptions.into_iter();
    slots
        .into_iter()
        .filter_map(|slot| match slot {
            Slot::Settled(hit) => hit,
            Slot::Pending { path, bytes } => {
                let description = descriptions.next().flatten();
                hit_for(&bytes, &path, place, description)
            }
        })
        .collect()
}

fn hit_for(bytes: &[u8], path: &Path, place: Place, description: Option<Vec<u8>>) -> Option<Hit> {
    message_for(bytes, path, place, description).map(|message| Hit {
        message,
        path: path.to_path_buf(),
    })
}

/// Whether [`message_for`] reaches for `file`'s description of these bytes.
fn needs_description(bytes: &[u8], path: &Path, place: Place) -> bool {
    if matches!(place, Place::Debian) && under(path, &["debian", "patches"]) {
        return !(bytes.is_empty() || std::str::from_utf8(dep3_header(bytes)).is_ok());
    }
    if std::str::from_utf8(bytes).is_ok() {
        return false;
    }
    !(matches!(place, Place::Control) && Hashbang::is_script(bytes))
}

fn is_content_file(entry: &Entry) -> bool {
    matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
}

/// The Hit message for one file. `description` is `file`'s output for `bytes`;
/// it is consulted only where [`needs_description`] says it is needed.
fn message_for(
    bytes: &[u8],
    path: &Path,
    place: Place,
    description: Option<Vec<u8>>,
) -> Option<&'static str> {
    if matches!(place, Place::Debian) && under(path, &["debian", "patches"]) {
        if bytes.is_empty() || std::str::from_utf8(dep3_header(bytes)).is_ok() {
            return None;
        }
        return ends_with_text(&description?).then_some("DEP-3 header");
    }
    if std::str::from_utf8(bytes).is_ok() {
        return None;
    }
    if matches!(place, Place::Control) && Hashbang::is_script(bytes) {
        return Some("");
    }
    let description = description?;
    if ends_with_text(&description) {
        return Some("");
    }
    if matches!(place, Place::Installed)
        && under(path, &["usr", "share", "man"])
        && contains_gzip(&description)
        && crate::lint::gzip::gunzip(bytes)
            .is_some_and(|plain| std::str::from_utf8(&plain).is_err())
    {
        return Some("");
    }
    None
}

/// Bytes before the first line that starts with `---`. The whole buffer when
/// there is no such line.
fn dep3_header(bytes: &[u8]) -> &[u8] {
    let mut offset = 0;
    while offset < bytes.len() {
        if bytes[offset..].starts_with(b"---") {
            return &bytes[..offset];
        }
        match bytes[offset..].iter().position(|byte| *byte == b'\n') {
            Some(index) => offset += index + 1,
            None => break,
        }
    }
    bytes
}

fn under(path: &Path, prefix: &[&str]) -> bool {
    let mut components = path.components();
    prefix.iter().all(|expected| {
        matches!(
            components.next(),
            Some(Component::Normal(name)) if name == OsStr::new(expected)
        )
    }) && components.next().is_some()
}

fn ends_with_text(description: &[u8]) -> bool {
    trim_ascii_end(description).ends_with(b"text")
}

fn contains_gzip(description: &[u8]) -> bool {
    description
        .windows(b"gzip compressed".len())
        .any(|window| window == b"gzip compressed")
}

fn trim_ascii_end(bytes: &[u8]) -> &[u8] {
    let end = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map_or(0, |index| index + 1);
    &bytes[..end]
}

/// Buffers per `file` run. Keeps the argument list short.
const BATCH: usize = 512;

/// The `file` program: the absolute path when the host has one, else the bare
/// name, which then fails to spawn as it always did.
fn file_program() -> PathBuf {
    external_tool::find_cached("file").unwrap_or_else(|| PathBuf::from("file"))
}

/// lintian's `file --raw` description of each buffer, in order. Runs `file`
/// once per [`BATCH`] buffers instead of once per buffer. `None` where `file`
/// gives no output or cannot be run.
fn file_descriptions(buffers: &[&[u8]]) -> Vec<Option<Vec<u8>>> {
    buffers.chunks(BATCH).flat_map(batch_descriptions).collect()
}

fn batch_descriptions(buffers: &[&[u8]]) -> Vec<Option<Vec<u8>>> {
    let none = || vec![None; buffers.len()];
    let Ok(dir) = tempfile::tempdir() else {
        return none();
    };
    // Buffers are staged under their index, so no file name needs parsing.
    for (index, bytes) in buffers.iter().enumerate() {
        if fs::write(dir.path().join(index.to_string()), bytes).is_err() {
            return none();
        }
    }
    let names: Vec<String> = (0..buffers.len()).map(|index| index.to_string()).collect();
    let Ok(output) = Command::new(file_program())
        .args(["--raw", "--print0", "--"])
        .args(&names)
        .current_dir(dir.path())
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .env("LANGUAGE", "C")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    else {
        return none();
    };
    match parse_batch(&output.stdout, buffers.len()) {
        Some(descriptions) => descriptions
            .into_iter()
            .map(|description| (!description.is_empty()).then_some(description))
            .collect(),
        // Output that does not parse back to the buffers: ask one at a time.
        None => buffers
            .iter()
            .map(|bytes| file_description(bytes))
            .collect(),
    }
}

/// The descriptions in a `file --raw --print0` run over `count` staged
/// buffers named `0..count`. Each record is `name NUL ": " description`, where
/// the description ends with a newline and the next record's name follows it
/// directly. `None` when the output does not have that shape.
fn parse_batch(stdout: &[u8], count: usize) -> Option<Vec<Vec<u8>>> {
    let tokens: Vec<&[u8]> = stdout.split(|byte| *byte == 0).collect();
    if tokens.len() != count + 1 || tokens[0] != &b"0"[..] {
        return None;
    }
    (0..count)
        .map(|index| {
            let mut body = tokens[index + 1];
            if index + 1 < count {
                body = body.strip_suffix((index + 1).to_string().as_bytes())?;
                if !body.ends_with(b"\n") {
                    return None;
                }
            }
            body.strip_prefix(b": ").map(<[u8]>::to_vec)
        })
        .collect()
}

/// lintian's `file --raw` description of one buffer, through `file -` on
/// stdin. `None` when `file` produces no output.
fn file_description(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut child = Command::new(file_program())
        .args(["--raw", "--brief", "-"])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .env("LANGUAGE", "C")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let wrote = child
        .stdin
        .take()
        .is_some_and(|mut stdin| stdin.write_all(bytes).is_ok());
    let output = child.wait_with_output().ok()?;
    if !wrote || output.stdout.is_empty() {
        return None;
    }
    Some(output.stdout)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::file_type::FileType;
    use crate::lint::tester::{
        BinaryPackageCase, BinaryPackageTester, SourcePackageCase, SourcePackageTester,
        SourceTreeTester, TestFile,
    };

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    const LATIN1: &[u8] = b"caf\xe9\n";

    fn packaged(files: Vec<(&str, TestFile)>) -> Vec<(&str, TestFile)> {
        let mut tree = vec![("debian/control", TestFile::new(CONTROL))];
        tree.extend(files);
        tree
    }

    fn latin1(path: &str) -> Vec<(&str, TestFile)> {
        packaged(vec![(path, TestFile::from_bytes(LATIN1))])
    }

    fn gzip_bytes(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(bytes).expect("gzip");
        encoder.finish().expect("gzip")
    }

    #[test]
    fn batch_output_parses_back_to_each_description() {
        // Multi-line descriptions and an empty one: the names of the next
        // records are found by their position, not by splitting lines.
        let stdout = b"0\0: ISO-8859 text\n1\0: first\nsecond\n2\0: empty\n3\0: x\n";
        assert_eq!(
            parse_batch(stdout, 4),
            Some(vec![
                b"ISO-8859 text\n".to_vec(),
                b"first\nsecond\n".to_vec(),
                b"empty\n".to_vec(),
                b"x\n".to_vec(),
            ])
        );
        // A record count that does not match is not parsed.
        assert_eq!(parse_batch(stdout, 3), None);
        assert_eq!(parse_batch(b"0\0ISO text\n", 1), None);
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no debian"))],
            packaged(vec![("debian/NEWS", TestFile::new("café\n"))]),
            packaged(vec![("README", TestFile::from_bytes(LATIN1))]),
            packaged(vec![(
                "debian/preinst",
                TestFile::from_bytes(b"#!/bin/sh\ncaf\xe9\n"),
            )]),
            packaged(vec![(
                "debian/rules",
                TestFile::from_bytes(b"#!/usr/bin/make -f\ncaf\xe9\n"),
            )]),
            packaged(vec![(
                "debian/lincity",
                TestFile::from_bytes(b"#!#!#!\ncaf\xe9\n"),
            )]),
            packaged(vec![("debian/NEWS", TestFile::from_bytes(b"caf\xe9\0\n"))]),
            packaged(vec![(
                "debian/logo",
                TestFile::from_bytes(b"\xff\xd8\xff\xe0\x00\x00\x00\x00\x00\x00caf\xe9"),
            )]),
            packaged(vec![(
                "debian/NEWS",
                TestFile::from_bytes(LATIN1).file_type(FileType::Elf {
                    has_static_symbol_table: false,
                    executable: true,
                }),
            )]),
            packaged(vec![("debian/NEWS", TestFile::from_bytes(b"caf\xe9\r\n"))]),
            packaged(vec![("debian/NEWS", TestFile::from_bytes(b"caf\xe9"))]),
            packaged(vec![("debian/NEWS", TestFile::from_bytes(b"\xe9"))]),
            packaged(vec![(
                "debian/NEWS",
                TestFile::from_bytes({
                    let mut bytes = vec![b'a'; 300];
                    bytes.extend_from_slice(b"\xe9\n");
                    bytes
                }),
            )]),
            packaged(vec![(
                "debian/NEWS",
                TestFile::from_bytes(b"hello\x1b[31mcaf\xe9\n"),
            )]),
            packaged(vec![("debian/NEWS", TestFile::from_bytes(b""))]),
            packaged(vec![("debian/NEWS", TestFile::symlink("missing"))]),
            packaged(vec![("debian/empty", TestFile::directory())]),
            packaged(vec![(
                "debian/patches/utf8.patch",
                TestFile::from_bytes(
                    b"Description: ok\nAuthor: me\n\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-\xe9\n",
                ),
            )]),
        ];
        let fail = vec![
            latin1("debian/NEWS"),
            latin1("debian/changelog"),
            latin1("debian/copyright"),
            latin1("debian/templates"),
            latin1("debian/tests/control"),
            latin1("debian/doc-base"),
            vec![("debian/control", TestFile::from_bytes(LATIN1))],
            packaged(vec![(
                "debian/NEWS",
                TestFile::from_bytes(LATIN1).hardlink(),
            )]),
            packaged(vec![
                ("debian/NEWS", TestFile::from_bytes(LATIN1)),
                ("debian/changelog", TestFile::from_bytes(LATIN1)),
            ]),
            packaged(vec![(
                "debian/patches/utf8.patch",
                TestFile::from_bytes(
                    b"Description: ok\nAuthor: caf\xe9\n\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n",
                ),
            )]),
            packaged(vec![(
                "debian/patches/series",
                TestFile::from_bytes(b"Description: caf\xe9\n"),
            )]),
        ];
        SourceTreeTester::new(NationalEncoding, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(packaged(vec![(
            "debian/NEWS",
            TestFile::new("café\n"),
        )]))];
        let fail = vec![
            SourcePackageCase::from(latin1("debian/NEWS")),
            SourcePackageCase::from(packaged(vec![(
                "debian/patches/utf8.patch",
                TestFile::from_bytes(
                    b"Description: ok\nAuthor: caf\xe9\n\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n",
                ),
            )])),
        ];
        SourcePackageTester::new(NationalEncoding, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/readme",
                TestFile::new("café\n"),
            )]),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("preinst", TestFile::new("#!/bin/sh\ntrue\n")),
            BinaryPackageCase::from(vec![(
                "usr/bin/example",
                TestFile::from_bytes(b"#!/bin/sh\ncaf\xe9\n"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/readme.gz",
                TestFile::from_bytes(gzip_bytes(LATIN1)),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/man/man1/example.1.gz",
                TestFile::from_bytes(gzip_bytes(b"hello\n")),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/logo",
                TestFile::from_bytes(b"\xff\xd8\xff\xe0\x00\x00\x00\x00\x00\x00caf\xe9"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/readme",
                TestFile::symlink("other"),
            )]),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("templates", TestFile::from_bytes(b"\xe9")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("templates", TestFile::from_bytes(b"caf\xe9\r\n")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("config", TestFile::from_bytes(b"#!#!#!\ncaf\xe9\n")),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/readme",
                TestFile::from_bytes(LATIN1),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/readme",
                TestFile::from_bytes(LATIN1).hardlink(),
            )]),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("templates", TestFile::from_bytes(LATIN1)),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("preinst", TestFile::from_bytes(b"#!/bin/sh\ncaf\xe9\n")),
            BinaryPackageCase::from(vec![(
                "usr/share/man/man1/example.1.gz",
                TestFile::from_bytes(gzip_bytes(LATIN1)),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/man/man1/example.1",
                TestFile::from_bytes(LATIN1),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/readme",
                TestFile::from_bytes(LATIN1),
            )])
            .udeb(),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/readme",
                TestFile::from_bytes(LATIN1),
            )])
            .control_file("templates", TestFile::from_bytes(LATIN1)),
        ];
        BinaryPackageTester::new(NationalEncoding, pass, fail)
            .test_and_snapshot_with_suffix("binary_package");
    }
}
