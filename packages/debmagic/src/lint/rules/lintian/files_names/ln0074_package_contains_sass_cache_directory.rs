use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::flag_name;
use super::names::NameHit;

pub struct PackageContainsSassCacheDirectory;

declare_rule! {
    /// The package ships a Sass cache directory.
    ///
    /// The path contains `/.sass-cache/`. The cache directory itself is
    /// included. Udebs are checked too. Extra is empty; the path (with a
    /// trailing slash on directories) is the Location.
    PackageContainsSassCacheDirectory,
    code = "LN0074",
    tag = "package-contains-sass-cache-directory",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageContainsSassCacheDirectory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_name(ctx, NameHit::Sass, Severity::Warning);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn files(entries: &[(&str, bool)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            entries
                .iter()
                .map(|(path, dir)| {
                    let file = if *dir {
                        TestFile::directory()
                    } else {
                        TestFile::new("x")
                    };
                    (path.to_string(), file)
                })
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            files(&[]),
            files(&[("usr/share/doc/pkg/.sass-cache", false)]),
        ];
        let fail = vec![
            files(&[
                (
                    "usr/share/doc/sass-cache-directory/.sass-cache/nothing",
                    false,
                ),
                ("usr/share/doc/sass-cache-directory/.sass-cache", true),
            ]),
            files(&[("usr/share/doc/pkg/.sass-cache", true)]).udeb(),
        ];
        BinaryPackageTester::new(PackageContainsSassCacheDirectory, pass, fail).test_and_snapshot();
    }
}
