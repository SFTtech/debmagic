use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct PerlModuleInCoreDirectory;

declare_rule! {
    /// A Data files entry sits below the core Perl module directories
    /// `usr/share/perl/` or `usr/lib/perl/`.
    ///
    /// Every entry whose path starts with one of those directories, followed by
    /// a non-space character, is reported. Directories count too. The vendor
    /// directories are the place for packaged modules. Nothing is reported when
    /// the Source package is `perl` itself. Reported at the path with an empty
    /// extra. Udebs are checked too.
    PerlModuleInCoreDirectory,
    code = "LN0818",
    tag = "perl-module-in-core-directory",
    default_selected = true,
    experimental = false,
}

static CORE_DIRECTORY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/(?:share|lib)/perl/\S").expect("valid regex"));

impl BinaryPackageRule for PerlModuleInCoreDirectory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.source_name() == "perl" {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| {
                entry
                    .path()
                    .to_str()
                    .is_some_and(|name| CORE_DIRECTORY.is_match(name))
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
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
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(
            files
                .into_iter()
                .map(|(path, file)| (path.to_string(), file))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            tree(vec![("usr/lib/perl5/Foo/Bar.pm", TestFile::new("x"))]),
            tree(vec![("usr/share/perl5/Foo/Bar.pm", TestFile::new("x"))]),
            tree(vec![("usr/lib/perl-foo/perllocal.pod", TestFile::new("x"))]),
            // The bare directories are not below.
            tree(vec![("usr/share/perl", TestFile::directory())]),
            tree(vec![("usr/lib/perl", TestFile::directory())]),
            tree(vec![("usr/share/perl-foo/bar", TestFile::new("x"))]),
            // The Source package `perl` is exempt.
            tree(vec![("usr/share/perl/bar", TestFile::new("x"))]).field("Source", "perl"),
        ];
        let fail = vec![
            tree(vec![("usr/share/perl/bar", TestFile::new("x"))]),
            tree(vec![("usr/lib/perl/bar", TestFile::new("x"))]),
            tree(vec![("usr/share/perl/5.36", TestFile::directory())]),
            tree(vec![("usr/share/perl/bar", TestFile::symlink("elsewhere"))]),
            tree(vec![("usr/share/perl/bar", TestFile::new("x"))]).field("Source", "perl-foo"),
            tree(vec![("usr/share/perl/bar", TestFile::new("x"))]).udeb(),
        ];
        BinaryPackageTester::new(PerlModuleInCoreDirectory, pass, fail).test_and_snapshot();
    }
}
