use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::{Severity, SourceRule};

pub struct OlderSourceFormat;

declare_rule! {
    /// The package uses source format `1.0`.
    ///
    /// Reported when `debian/source/format` is not a regular file (missing, or
    /// a symlink, as lintian's `is_open_ok` sees it: the format then counts as
    /// the implicit `1.0`), or when its first line, without the newline, is
    /// exactly `1.0`. Extra is `1.0`; the Location is empty. A format file
    /// that is empty or names an unknown format makes lintian's Check die, so
    /// nothing is reported for it. Applies to the Source tree and Source
    /// package.
    OlderSourceFormat,
    code = "LN1335",
    tag = "older-source-format",
    default_selected = false,
    experimental = false,
}

impl SourceRule for OlderSourceFormat {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let format = match files.get("debian/source/format") {
            Some(entry) if entry.kind() == EntryKind::RegularFile => {
                let Ok(bytes) = files.read(entry) else {
                    return;
                };
                let first = bytes.split_inclusive(|&byte| byte == b'\n').next();
                let Some(first) = first else {
                    return;
                };
                first.strip_suffix(b"\n").unwrap_or(first).to_vec()
            }
            _ => b"1.0".to_vec(),
        };
        if format == b"1.0" {
            ctx.diagnostic(Diagnostic::warn("1.0").with_severity(Severity::Info));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn format(content: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/source/format", TestFile::new(content))]
    }

    #[test]
    fn test() {
        let pass = vec![
            format("3.0 (quilt)\n"),
            format("3.0 (native)"),
            format("2.0\n"),
            // Empty: lintian dies on it, so nothing is reported.
            format(""),
            // Unknown formats die too.
            format("1.0-old\n"),
            format("1.0\r\n"),
            // Only the first line counts.
            format("3.0 (quilt)\n1.0\n"),
        ];
        let fail = vec![
            format("1.0\n"),
            format("1.0"),
            format("1.0\nforeign\n"),
            // A missing format is the implicit 1.0.
            vec![("README", TestFile::new("no packaging"))],
            // A misspelled name does not count.
            vec![("debian/source/formt", TestFile::new("3.0 (quilt)\n"))],
            // A directory or a symlink is not a regular file.
            vec![("debian/source/format", TestFile::directory())],
            vec![("debian/source/format", TestFile::symlink("/etc/hostname"))],
        ];
        SourceTreeTester::new(OlderSourceFormat, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(format("3.0 (quilt)\n"))];
        let fail = vec![SourcePackageCase::from(format("1.0\n"))];
        SourcePackageTester::new(OlderSourceFormat, pass, fail).test_and_snapshot();
    }
}
