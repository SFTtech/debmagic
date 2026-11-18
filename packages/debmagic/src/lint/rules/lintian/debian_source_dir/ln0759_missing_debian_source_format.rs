use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

pub struct MissingDebianSourceFormat;

declare_rule! {
    /// `debian/source/format` is not a regular file.
    ///
    /// Reads the Source tree, or the Source package Patched files. Fires
    /// once when `debian/source/format` is missing, or is not a regular file
    /// (a symlink is not followed). Extra and Location are empty.
    MissingDebianSourceFormat,
    code = "LN0759",
    tag = "missing-debian-source-format",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MissingDebianSourceFormat {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let present = files
            .get("debian/source/format")
            .is_some_and(|entry| entry.kind() == EntryKind::RegularFile);
        if !present {
            ctx.diagnostic(Diagnostic::warn(""));
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
            format("1.0\n"),
            // An empty format file is still a file.
            format(""),
        ];
        let fail = vec![
            vec![("README", TestFile::new("no packaging"))],
            // A misspelled name does not count.
            vec![("debian/source/formt", TestFile::new("3.0 (quilt)\n"))],
            vec![("debian/source/format", TestFile::directory())],
            vec![("debian/source/format", TestFile::symlink("/etc/hostname"))],
        ];
        SourceTreeTester::new(MissingDebianSourceFormat, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(format("3.0 (quilt)\n"))];
        let fail = vec![SourcePackageCase::from(vec![(
            "debian/source/formt",
            TestFile::new("3.0 (quilt)\n"),
        )])];
        SourcePackageTester::new(MissingDebianSourceFormat, pass, fail).test_and_snapshot();
    }
}
