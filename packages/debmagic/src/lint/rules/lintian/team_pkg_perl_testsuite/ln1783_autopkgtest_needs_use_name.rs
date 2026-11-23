use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::SourceRule;

use super::{is_pkg_perl, source_fields, uses_pkg_perl_testsuite};

pub struct AutopkgtestNeedsUseName;

declare_rule! {
    /// A pkg-perl package uses `autopkgtest-pkg-perl` but its `use.t` has no
    /// module name to load.
    ///
    /// Applies when the Maintainer contains
    /// `pkg-perl-maintainers@lists.alioth.debian.org` and `Testsuite` names
    /// `autopkgtest-pkg-perl`. The test is satisfied by a non-empty `META.json`
    /// or `META.yml` at the top of the patched tree (a regular file or hardlink
    /// with bytes; a symbolic link or directory has size zero for lintian), or
    /// by any entry at `debian/tests/pkg-perl/use-name`. Extra and Location are
    /// empty. Source package: the `.dsc` fields; Source tree: the source
    /// paragraph of `debian/control`.
    AutopkgtestNeedsUseName,
    code = "LN1783",
    tag = "autopkgtest-needs-use-name",
    default_selected = true,
    experimental = false,
}

/// Lintian's `size` of an item at the top of the tree: bytes of a regular file.
fn has_content(files: &FileTree, path: &str) -> bool {
    let Some(entry) = files.get(path) else {
        return false;
    };
    matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
        && files.read(entry).is_ok_and(|bytes| !bytes.is_empty())
}

impl SourceRule for AutopkgtestNeedsUseName {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let applies = {
            let Some(fields) = source_fields(ctx) else {
                return;
            };
            is_pkg_perl(fields) && uses_pkg_perl_testsuite(fields)
        };
        if !applies {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        let named = has_content(files, "META.json")
            || has_content(files, "META.yml")
            || files.get("debian/tests/pkg-perl/use-name").is_some();
        if !named {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const PKG_PERL: &str = "Debian Perl Group <pkg-perl-maintainers@lists.alioth.debian.org>";

    fn control(maintainer: &str, testsuite: &str) -> (&'static str, TestFile) {
        (
            "debian/control",
            TestFile::new(format!(
                "Source: libfoo-perl\nMaintainer: {maintainer}\nTestsuite: {testsuite}\n\
                 Standards-Version: 4.7.2\nSection: perl\n\
                 \nPackage: libfoo-perl\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )
    }

    fn tree(maintainer: &str, testsuite: &str) -> Vec<(&'static str, TestFile)> {
        vec![control(maintainer, testsuite)]
    }

    fn with(
        mut files: Vec<(&'static str, TestFile)>,
        extra: (&'static str, TestFile),
    ) -> Vec<(&'static str, TestFile)> {
        files.push(extra);
        files
    }

    #[test]
    fn source_tree() {
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            tree("Someone <someone@example.com>", "autopkgtest-pkg-perl"),
            tree(PKG_PERL, "autopkgtest"),
            with(
                tree(PKG_PERL, "autopkgtest-pkg-perl"),
                ("META.json", TestFile::new("{\"name\": \"Foo\"}\n")),
            ),
            with(
                tree(PKG_PERL, "autopkgtest-pkg-perl"),
                ("META.yml", TestFile::new("name: Foo\n")),
            ),
            with(
                tree(PKG_PERL, "autopkgtest, autopkgtest-pkg-perl"),
                ("debian/tests/pkg-perl/use-name", TestFile::new("")),
            ),
        ];
        let fail = vec![
            tree(PKG_PERL, "autopkgtest-pkg-perl"),
            with(
                tree(PKG_PERL, "autopkgtest-pkg-perl"),
                ("META.json", TestFile::new("")),
            ),
            with(
                tree(PKG_PERL, "autopkgtest-pkg-perl"),
                ("META.yml", TestFile::new("")),
            ),
            // Lintian's size of a symbolic link or directory is zero.
            with(
                tree(PKG_PERL, "autopkgtest-pkg-perl"),
                ("META.json", TestFile::symlink("README")),
            ),
            with(
                tree(PKG_PERL, "autopkgtest-pkg-perl"),
                ("META.json", TestFile::directory()),
            ),
        ];
        SourceTreeTester::new(AutopkgtestNeedsUseName, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let bare = Vec::<(&str, TestFile)>::new();
        let dsc = |maintainer: &str| {
            Dsc::parse(&format!(
                "Source: libfoo-perl\nMaintainer: {maintainer}\nTestsuite: autopkgtest-pkg-perl\n"
            ))
            .expect("Dsc fixture")
        };
        let pass = vec![
            SourcePackageCase::from(vec![("META.json", TestFile::new("{}\n"))]).dsc(dsc(PKG_PERL)),
            SourcePackageCase::from(bare.clone()).dsc(dsc("Someone <someone@example.com>")),
        ];
        let fail = vec![SourcePackageCase::from(bare).dsc(dsc(PKG_PERL))];
        SourcePackageTester::new(AutopkgtestNeedsUseName, pass, fail).test_and_snapshot();
    }
}
