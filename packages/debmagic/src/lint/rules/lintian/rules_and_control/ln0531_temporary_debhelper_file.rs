use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct TemporaryDebhelperFile;

declare_rule! {
    /// `debian/` contains a file debhelper leaves behind while building.
    ///
    /// Any direct child of `debian/` (file, directory, or link) whose name is
    /// `debhelper` or `debhelper.log` after an optional dotted prefix, such as
    /// `package.postinst.debhelper` or `package.debhelper.log`, reports. A
    /// child named exactly `debhelper` does not: debhelper never makes it.
    /// Unlike the other debhelper Tags this does not need `debian/rules`. Extra
    /// is empty. Location is the path with no line. Applies to the Source tree
    /// and Source package.
    TemporaryDebhelperFile,
    code = "LN0531",
    tag = "temporary-debhelper-file",
    default_selected = true,
    experimental = false,
}

static TEMPORARY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|\.)debhelper(?:\.log)?$").expect("valid regex"));

impl SourceRule for TemporaryDebhelperFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let found: Vec<PathBuf> = files
            .entries()
            .iter()
            .map(|entry| entry.path())
            .filter(|path| path.parent() == Some(Path::new("debian")))
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name != "debhelper" && TEMPORARY.is_match(name))
            })
            .map(Path::to_path_buf)
            .collect();
        for path in found {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            vec![("debian/control", TestFile::new("Source: example\n"))],
            // debhelper does not make a file of exactly this name.
            vec![("debian/debhelper", TestFile::new(""))],
            vec![("debian/package.postinst", TestFile::new(""))],
            vec![("debian/package.debhelperx", TestFile::new(""))],
            vec![("debian/xdebhelper", TestFile::new(""))],
            vec![("debian/debhelper.logx", TestFile::new(""))],
            // Only direct children of `debian/`.
            vec![("debian/sub/package.debhelper.log", TestFile::new(""))],
            vec![("package.debhelper.log", TestFile::new(""))],
        ];
        let fail = vec![
            vec![("debian/package.debhelper.log", TestFile::new(""))],
            vec![
                ("debian/package.postinst.debhelper", TestFile::new("")),
                ("debian/package.prerm.debhelper", TestFile::new("")),
            ],
            vec![("debian/.debhelper", TestFile::directory())],
            vec![("debian/debhelper.log", TestFile::new(""))],
            vec![("debian/.debhelper.log", TestFile::new(""))],
            vec![("debian/link.debhelper", TestFile::symlink("elsewhere"))],
        ];
        SourceTreeTester::new(TemporaryDebhelperFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(vec![(
            "debian/control",
            TestFile::new("Source: example\n"),
        )])];
        let fail = vec![SourcePackageCase::from(vec![(
            "debian/package.postinst.debhelper",
            TestFile::new(""),
        )])];
        SourcePackageTester::new(TemporaryDebhelperFile, pass, fail).test_and_snapshot();
    }
}
