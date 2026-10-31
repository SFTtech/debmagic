use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::gzip::gunzip;
use crate::lint::rule::BinaryPackageRule;

pub struct ReadmeDebianMentionsUsrDoc;

declare_rule! {
    /// `README.Debian` mentions the retired `/usr/doc` path.
    ///
    /// The first readable file of `README.Debian.gz`, `README.Debian`,
    /// `README.debian.gz`, and `README.debian` directly under
    /// `usr/share/doc/<package>/` is scanned. Gzip names are decompressed.
    /// `/usr/doc` must sit on a word boundary, once per physical line. Extra
    /// is empty. Location is that file and line.
    ReadmeDebianMentionsUsrDoc,
    code = "LN0335",
    tag = "readme-debian-mentions-usr-doc",
    default_selected = true,
    experimental = false,
}

const README_NAMES: [&str; 4] = [
    "README.Debian.gz",
    "README.Debian",
    "README.debian.gz",
    "README.debian",
];

static USR_DOC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)/usr/doc\b").expect("valid regex"));

impl BinaryPackageRule for ReadmeDebianMentionsUsrDoc {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let hits = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            readme_hits(files, &package)
        };
        for (path, line) in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

fn readme_hits(files: &FileTree, package: &str) -> Vec<(PathBuf, u32)> {
    let Some((path, text)) = readme_text(files, package) else {
        return Vec::new();
    };
    physical_lines(&text)
        .into_iter()
        .enumerate()
        .filter(|(_, line)| USR_DOC.is_match(line))
        .map(|(index, _)| (path.clone(), u32::try_from(index + 1).unwrap_or(u32::MAX)))
        .collect()
}

fn readme_text(files: &FileTree, package: &str) -> Option<(PathBuf, String)> {
    for name in README_NAMES {
        let path = PathBuf::from(format!("usr/share/doc/{package}/{name}"));
        let Some(entry) = files.get(&path) else {
            continue;
        };
        if !is_file(entry) {
            continue;
        }
        let bytes = files.read(entry).ok()?;
        let text = if name.ends_with(".gz") {
            let plain = gunzip(&bytes)?;
            String::from_utf8_lossy(&plain).into_owned()
        } else {
            String::from_utf8_lossy(&bytes).into_owned()
        };
        return Some((path, text));
    }
    None
}

fn is_file(entry: &Entry) -> bool {
    matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
}

fn physical_lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<&str> = text.split('\n').collect();
    if text.ends_with('\n') {
        lines.pop();
    }
    lines
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn readme(package: &str, name: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            format!("usr/share/doc/{package}/{name}"),
            TestFile::new(body),
        )])
        .package(package)
    }

    fn gzip_bytes(body: &str) -> TestFile {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(body.as_bytes()).expect("gzip");
        TestFile::from_bytes(encoder.finish().expect("gzip finish"))
    }

    fn gzip_readme(package: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            format!("usr/share/doc/{package}/README.Debian.gz"),
            gzip_bytes(body),
        )])
        .package(package)
    }

    /// Gzip is preferred over a plain `README.Debian` in the same directory.
    fn gzip_and_plain(package: &str, gz_body: &str, plain: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![
            (
                format!("usr/share/doc/{package}/README.Debian.gz"),
                gzip_bytes(gz_body),
            ),
            (
                format!("usr/share/doc/{package}/README.Debian"),
                TestFile::new(plain),
            ),
        ])
        .package(package)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            readme(
                "example",
                "README.Debian",
                "See /usr/documentation and /usr/share/doc and /usr/doc2.\n",
            ),
            gzip_readme("example", "Nothing about the old path.\n"),
            gzip_and_plain(
                "example",
                "Nothing about the old path.\n",
                "The plain file mentions /usr/doc.\n",
            ),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/README.Debian",
                TestFile::new("See /usr/doc/foo.\n"),
            )])
            .package("other"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/README.Debian",
                TestFile::symlink("missing"),
            )]),
        ];
        let fail = vec![
            readme(
                "etcfiles",
                "README.Debian",
                "this is a package to test lintian's handling of files in /etc.\nAlso, there's a random mention of /usr/doc here to prompt a warning.\nBut /usr/documentation doesn't.\n",
            ),
            readme(
                "debian-readme-general",
                "README.Debian",
                "This is a test README.Debian with a few problems.\n\n\nHere is a reference to /usr/doc/debian-readme-general, which is an old\npath.\n",
            ),
            readme(
                "example",
                "README.debian",
                "first\n/usr/doc/\n/usr/doc again\n",
            ),
            gzip_readme("example", "line\n/usr/doc\n"),
            gzip_and_plain("example", "mentions /usr/doc\n", "plain has no old path\n"),
        ];
        BinaryPackageTester::new(ReadmeDebianMentionsUsrDoc, pass, fail).test_and_snapshot();
    }
}
