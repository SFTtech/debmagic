use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
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

fn inspect(files: &FileTree, place: Place) -> Vec<Hit> {
    let mut hits = Vec::new();
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
        if let Some(message) = message_for(&bytes, path, place) {
            hits.push(Hit {
                message,
                path: path.to_path_buf(),
            });
        }
    }
    hits
}

fn is_content_file(entry: &Entry) -> bool {
    matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
}

fn message_for(bytes: &[u8], path: &Path, place: Place) -> Option<&'static str> {
    if matches!(place, Place::Debian) && under(path, &["debian", "patches"]) {
        if bytes.is_empty() || std::str::from_utf8(dep3_header(bytes)).is_ok() {
            return None;
        }
        return ends_with_text(&file_description(bytes)?).then_some("DEP-3 header");
    }
    if std::str::from_utf8(bytes).is_ok() {
        return None;
    }
    if matches!(place, Place::Control) && is_script(bytes) {
        return Some("");
    }
    let description = file_description(bytes)?;
    if ends_with_text(&description) {
        return Some("");
    }
    if matches!(place, Place::Installed)
        && under(path, &["usr", "share", "man"])
        && contains_gzip(&description)
        && gunzip(bytes).is_some_and(|plain| std::str::from_utf8(&plain).is_err())
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

/// `#!`, except the lincity data magic `#!#!#!`.
fn is_script(bytes: &[u8]) -> bool {
    bytes.starts_with(b"#!") && !bytes.starts_with(b"#!#!#!")
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

/// lintian's `file --raw` description. `None` when `file` produces no output.
fn file_description(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut child = Command::new("file")
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

fn gunzip(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = flate2::read::GzDecoder::new(bytes);
    let mut plain = Vec::new();
    decoder.read_to_end(&mut plain).ok()?;
    Some(plain)
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
