use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;
use regex::bytes::Regex as BytesRegex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;
use debmagic_common::compression::gunzip;

use super::scan;
use crate::lint::file_tree::file_name_lossy;

static INFO_GZ: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\.info(?:-\d+)?\.gz$").expect("valid regex"));

/// Texinfo `@image` in the `.info` form: a NUL and a backspace, then `[image src="..."`.
static IMAGE: LazyLock<BytesRegex> = LazyLock::new(|| {
    BytesRegex::new(r#"(?s-u)\x00\x08\[image src="((?:\\.|[^"])+)""#).expect("valid regex")
});

pub struct InfoDocumentMissingImageFile;

declare_rule! {
    /// A gzipped info document (`*.info.gz` or `*.info-<N>.gz`) has an
    /// `[image src="..."]` form whose file is not in the package. `src` is
    /// resolved against `usr/share/info`. The extra is the `src`, and the line
    /// is the line of the form. Symlinks and `dir` files are skipped.
    InfoDocumentMissingImageFile,
    code = "LN0751",
    tag = "info-document-missing-image-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InfoDocumentMissingImageFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut missing: Vec<(PathBuf, u32, String)> = Vec::new();
        for entry in scan::info_items(files) {
            if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
                continue;
            }
            let basename = file_name_lossy(entry.path());
            if !INFO_GZ.is_match(&basename) {
                continue;
            }
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            let Some(plain) = gunzip(&bytes) else {
                continue;
            };
            for (index, line) in plain.split_inclusive(|&byte| byte == b'\n').enumerate() {
                let line_number = u32::try_from(index + 1).unwrap_or(u32::MAX);
                let mut reported: Vec<String> = Vec::new();
                for captures in IMAGE.captures_iter(line) {
                    let src = unescape(&captures[1]);
                    let found = scan::normalize_link_target("usr/share/info", &src)
                        .is_some_and(|path| files.get(&path).is_some());
                    if found || reported.contains(&src) {
                        continue;
                    }
                    reported.push(src.clone());
                    missing.push((entry.path().to_path_buf(), line_number, src));
                }
            }
        }
        for (path, line, src) in missing {
            ctx.diagnostic(Diagnostic::warn(src).with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

/// Drops the backslash of each `\X` pair.
fn unescape(src: &[u8]) -> String {
    let mut out = Vec::with_capacity(src.len());
    let mut bytes = src.iter().copied();
    while let Some(byte) = bytes.next() {
        if byte == b'\\' {
            if let Some(next) = bytes.next() {
                out.push(next);
            }
        } else {
            out.push(byte);
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn gzip(bytes: &[u8]) -> TestFile {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(bytes).expect("gzip");
        TestFile::from_bytes(encoder.finish().expect("gzip"))
    }

    /// Six plain lines, then line 7 with one `[image]` form per `src`.
    fn document(srcs: &[&str]) -> Vec<u8> {
        let mut text = "plain\n".repeat(6).into_bytes();
        for src in srcs {
            text.extend_from_slice(format!("\x00\x08[image src=\"{src}\"]").as_bytes());
        }
        text.push(b'\n');
        text
    }

    /// A document at `usr/share/info/<name>` with the `extra` files beside it.
    fn case(name: &str, content: TestFile, extra: &[&str]) -> BinaryPackageCase {
        let mut files = vec![(format!("usr/share/info/{name}"), content)];
        files.extend(
            extra
                .iter()
                .map(|path| (path.to_string(), TestFile::new("png"))),
        );
        BinaryPackageCase::from(files)
    }

    #[test]
    fn test() {
        let pass = vec![
            case(
                "w.info.gz",
                gzip(&document(&["foo.png"])),
                &["usr/share/info/foo.png"],
            ),
            case(
                "w.info.gz",
                gzip(&document(&["sub/../foo.png"])),
                &["usr/share/info/foo.png"],
            ),
            case(
                "w.info.gz",
                gzip(&document(&["/usr/share/info/foo.png"])),
                &["usr/share/info/foo.png"],
            ),
            case("w.info", TestFile::from_bytes(document(&["foo.png"])), &[]),
            case(
                "w.info.gz",
                TestFile::from_bytes(document(&["foo.png"])),
                &[],
            ),
            case("dirw.info.gz", gzip(&document(&["foo.png"])), &[]),
            case("w.newinfo.gz", gzip(&document(&["foo.png"])), &[]),
            case("w.info.gz", TestFile::symlink("x.info.gz"), &[]),
        ];
        let fail = vec![
            case("w.info.gz", gzip(&document(&["some-image.png"])), &[]),
            case(
                "w.info-1.gz",
                gzip(&document(&["foo.png", "bar.png"])),
                &["usr/share/info/foo.png"],
            ),
            case("w.info.gz", gzip(&document(&["x.png", "x.png"])), &[]),
            case("w.info.gz", gzip(&document(&["../foo.png"])), &[]),
            case("w.info.gz", gzip(&document(&["../../../x.png"])), &[]),
        ];
        BinaryPackageTester::new(InfoDocumentMissingImageFile, pass, fail).test_and_snapshot();
    }
}
