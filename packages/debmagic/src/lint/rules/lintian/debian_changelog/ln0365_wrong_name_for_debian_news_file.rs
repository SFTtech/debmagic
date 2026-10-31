use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::installed::wrong_debian_news_paths;

pub struct WrongNameForDebianNewsFile;

declare_rule! {
    /// A Debian news file is installed under a name other than `NEWS.Debian.gz`.
    ///
    /// Looks at children of `usr/share/doc/<pkg>` whose basename matches
    /// `NEWS.Debian` with an optional `.gz`, ignoring letter case. A compressed
    /// name that is not exactly `NEWS.Debian.gz` is reported, including
    /// `news.debian.gz`. `NEWS.Debian` without `.gz` is a different Tag. Extra
    /// is empty. The Location is the file. Udebs are not checked.
    WrongNameForDebianNewsFile,
    code = "LN0365",
    tag = "wrong-name-for-debian-news-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for WrongNameForDebianNewsFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths = wrong_debian_news_paths(files, &package);
        for path in paths {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
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

    fn packaged(name: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(name.to_string(), TestFile::new("news\n"))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            packaged("usr/share/doc/example/NEWS.Debian.gz"),
            packaged("usr/share/doc/example/NEWS.Debian"),
            packaged("usr/share/doc/example/NEWS.Debian.GZ"),
            packaged("usr/share/doc/example/news.debian"),
            packaged("usr/share/doc/example/subdir/news.debian.gz"),
            packaged("usr/share/doc/example/news.debian.gz").udeb(),
        ];
        let fail = vec![
            packaged("usr/share/doc/example/news.debian.gz"),
            packaged("usr/share/doc/example/NEWS.debian.gz"),
            packaged("usr/share/doc/example/News.Debian.gz"),
        ];
        BinaryPackageTester::new(WrongNameForDebianNewsFile, pass, fail).test_and_snapshot();
    }
}
