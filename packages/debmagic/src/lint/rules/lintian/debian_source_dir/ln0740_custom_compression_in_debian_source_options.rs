use std::path::PathBuf;
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

pub struct CustomCompressionInDebianSourceOptions;

declare_rule! {
    /// `debian/source/options` selects a compression level or algorithm.
    ///
    /// Reads `debian/source/options` when it is a regular file in the
    /// packaging File tree (Source tree, or Source package Patched files).
    /// Each line matching optional leading whitespace, `compression` or
    /// `compression-level`, optional whitespace, `=`, whitespace, and a
    /// non-whitespace run, followed by a newline, fires once. Extra is that
    /// text from `compression` on. Location is the file and the 1-based line.
    /// A symlink is not followed.
    CustomCompressionInDebianSourceOptions,
    code = "LN0740",
    tag = "custom-compression-in-debian-source-options",
    default_selected = true,
    experimental = false,
}

static COMPRESSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?-u)^\s*(compression(?:-level)?\s*=\s+\S+)\n").expect("valid regex")
});

impl SourceRule for CustomCompressionInDebianSourceOptions {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(entry) = files.get("debian/source/options") else {
            return;
        };
        if entry.kind() != EntryKind::RegularFile {
            return;
        }
        let Ok(bytes) = files.read(entry) else {
            return;
        };
        let hits: Vec<(u32, String)> = bytes
            .split_inclusive(|&byte| byte == b'\n')
            .enumerate()
            .filter_map(|(index, line)| {
                let captures = COMPRESSION.captures(line)?;
                let extra = String::from_utf8_lossy(&captures[1]).into_owned();
                Some((index as u32 + 1, extra))
            })
            .collect();
        for (line, extra) in hits {
            ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
                path: PathBuf::from("debian/source/options"),
                line: Some(line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn options(content: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/source/options", TestFile::new(content))]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            options("local-options\nno-pristine-tar\n"),
            // A comment is not a setting.
            options("# compression = xz\n"),
            // Without whitespace after `=`, or with a key that only starts alike.
            options("compression=xz\ncompression-level=9\n"),
            options("compression-x = 1\n"),
            // Trailing whitespace, a missing value, or no final newline.
            options("compression = xz \n"),
            options("compression =\n"),
            options("compression = xz"),
            // The value cannot continue on the next line.
            options("compression =\n  xz\n"),
            vec![("debian/source/options", TestFile::directory())],
        ];
        let fail = vec![
            options("compression = xz\n"),
            options("compression-level = 9\n"),
            options("  compression   =   xz\n"),
            options("compression = xz\n# keep\ncompression-level = 9\n"),
        ];
        SourceTreeTester::new(CustomCompressionInDebianSourceOptions, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(vec![(
            "debian/source/options",
            TestFile::new("local-options\n"),
        )])];
        let fail = vec![SourcePackageCase::from(vec![(
            "debian/source/options",
            TestFile::new("compression = xz\n"),
        )])];
        SourcePackageTester::new(CustomCompressionInDebianSourceOptions, pass, fail)
            .test_and_snapshot();
    }
}
