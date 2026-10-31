use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::flag_name;
use super::names::NameHit;

pub struct PackageContainsPythonDotDirectory;

declare_rule! {
    /// A Python dist-packages or site-packages tree contains a dot-directory.
    ///
    /// The parent directory must be `usr/lib/python<abi>/(dist|site)-packages/`,
    /// and the path must contain a `/.<name>/` segment. The dot-directory
    /// itself is included. Udebs are checked too. Extra is empty; the path
    /// (with a trailing slash on directories) is the Location.
    PackageContainsPythonDotDirectory,
    code = "LN0072",
    tag = "package-contains-python-dot-directory",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageContainsPythonDotDirectory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_name(ctx, NameHit::PythonDot, Severity::Warning);
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
            files(&[("usr/lib/python3/dist-packages/foo.py", false)]),
            files(&[("usr/lib/python3/dist-packages/.hidden", false)]),
            files(&[("usr/share/python3/dist-packages/foo/.directory/x", false)]),
        ];
        let fail = vec![
            files(&[
                ("usr/lib/python3/dist-packages/foo/.directory/foo", false),
                ("usr/lib/python3/dist-packages/foo/.directory/bar", false),
                ("usr/lib/python3/dist-packages/foo/.directory", true),
            ]),
            files(&[("usr/lib/python3/site-packages/pkg/.cache/x", false)]).udeb(),
        ];
        BinaryPackageTester::new(PackageContainsPythonDotDirectory, pass, fail).test_and_snapshot();
    }
}
