use std::path::PathBuf;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct ZeroByteFileInDocDirectory;

declare_rule! {
    /// A file under `usr/share/doc/` of the Binary package is empty.
    ///
    /// Walks the regular files of Data files under `usr/share/doc/`. An empty
    /// file is flagged unless it is in an `examples/` directory (directly under
    /// the doc root or one package level down), is a doxygen or html `*.map`
    /// file, or is an `__init__.py`. A `.gz` file of at most 276 bytes whose
    /// payload decompresses to nothing is flagged, examples included. Udebs are
    /// checked too. Extra is empty; the Data files path is the Location.
    ZeroByteFileInDocDirectory,
    code = "LN0713",
    tag = "zero-byte-file-in-doc-directory",
    default_selected = true,
    experimental = false,
}

/// lintian 2.139.0 `MAXIMUM_EMPTY_GZIP_SIZE`: compressed size bound for an empty payload.
const MAXIMUM_EMPTY_GZIP_SIZE: usize = 276;

const GZIP_MAGIC: &[u8] = &[0x1f, 0x8b];

static DOC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)^usr/share/doc/\S").expect("valid regex"));
static EXAMPLES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)^usr/share/doc/(?:[^/]+/)?examples/").expect("valid regex"));
static DOXYGEN_OR_HTML_MAP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s-u)^usr/share/doc/(?:.+/)?(?:doxygen|html)/.*[.]map$").expect("valid regex")
});
static INIT_PY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?-u)^usr/share/doc/(?:.+/)?__init__[.]py$").expect("valid regex")
});

impl BinaryPackageRule for ZeroByteFileInDocDirectory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .query()
            .kinds([EntryKind::RegularFile])
            .into_iter()
            .filter(|entry| DOC.is_match(entry.path().as_os_str().as_encoded_bytes()))
            .filter(|entry| {
                let Ok(bytes) = files.read(entry) else {
                    return false;
                };
                is_flagged(entry.path().as_os_str().as_encoded_bytes(), &bytes)
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// An empty file outside the exempt paths, or a small gzip file that
/// decompresses to nothing. `name` is a path under `usr/share/doc/`.
fn is_flagged(name: &[u8], bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return !(EXAMPLES.is_match(name)
            || DOXYGEN_OR_HTML_MAP.is_match(name)
            || INIT_PY.is_match(name));
    }
    name.ends_with(b".gz")
        && bytes.len() <= MAXIMUM_EMPTY_GZIP_SIZE
        && bytes.starts_with(GZIP_MAGIC)
        && debmagic_common::compression::gunzip(bytes).is_some_and(|plain| plain.is_empty())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::io::Write;
    use std::os::unix::ffi::OsStringExt;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn files(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::new("content")))
                .collect::<Vec<_>>(),
        )
    }

    fn empty(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::new("")))
                .collect::<Vec<_>>(),
        )
    }

    fn gzipped(content: &[u8]) -> TestFile {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(content).expect("gzip to memory");
        TestFile::from_bytes(encoder.finish().expect("gzip to memory"))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            files(&[
                "usr/share/doc/example/copyright",
                "usr/share/doc/example/changelog.gz",
            ]),
            // Empty files outside usr/share/doc are not documentation.
            empty(&[
                "usr/bin/example",
                "usr/share/man/man1/example.1.gz",
                "usr/share/docs/example",
            ]),
            // Examples are exempt, directly under the doc root or one package level down.
            empty(&[
                "usr/share/doc/example/examples/empty",
                "usr/share/doc/examples/empty",
            ]),
            // Doxygen and html maps, and Python package markers.
            empty(&[
                "usr/share/doc/example/html/blah.map",
                "usr/share/doc/example/doxygen/blah.map",
                "usr/share/doc/example/manual/1/html/foo/blah.map",
                "usr/share/doc/example/__init__.py",
                "usr/share/doc/example/python/foo/__init__.py",
            ]),
            // Directories and symlinks are not files.
            BinaryPackageCase::from(vec![
                ("usr/share/doc/example/empty-dir", TestFile::directory()),
                ("usr/share/doc/example/link", TestFile::symlink("copyright")),
            ]),
            // A gzip file whose payload is not empty, even when it is one newline.
            BinaryPackageCase::from(vec![
                ("usr/share/doc/example/NEWS.gz", gzipped(b"news\n")),
                ("usr/share/doc/example/NEWS.Debian.gz", gzipped(b"\n")),
            ]),
        ];
        let fail = vec![
            empty(&["usr/share/doc/example/NEWS"]),
            empty(&["usr/share/doc/README.Debian"]),
            // Only the exact directory and file names are exempt.
            empty(&[
                "usr/share/doc/example/html.map",
                "usr/share/doc/example/htmlfoo.map",
                "usr/share/doc/example/foo__init__.py",
                "usr/share/doc/example/html/blah.txt",
            ]),
            // Examples are exempt only one package level down.
            empty(&["usr/share/doc/a/b/examples/empty"]),
            // A zero-byte gzip file is flagged as an empty file.
            empty(&["usr/share/doc/example/empty.gz"]),
            // A gzip file with an empty payload is flagged, examples and maps included.
            BinaryPackageCase::from(vec![("usr/share/doc/example/NEWS.gz", gzipped(b""))]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/examples/very_interesting.gz",
                gzipped(b""),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/manual/foo.map.gz",
                gzipped(b""),
            )]),
            // A name that is not UTF-8 is still judged by its bytes.
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"usr/share/doc/example/bokm\xe5l".to_vec()),
                TestFile::new(""),
            )]),
            empty(&["usr/share/doc/example/NEWS"]).udeb(),
        ];
        BinaryPackageTester::new(ZeroByteFileInDocDirectory, pass, fail).test_and_snapshot();
    }
}
