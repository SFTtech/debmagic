use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::flag_name;
use super::names::NameHit;

pub struct PackageContainsPythonHypothesisExample;

declare_rule! {
    /// The package ships a Hypothesis example cache.
    ///
    /// The entry's directory contains `/.hypothesis/examples/`. The `show`
    /// directory under that cache is included. Udebs are checked too. Extra
    /// is empty; the path (with a trailing slash on directories) is the
    /// Location.
    PackageContainsPythonHypothesisExample,
    code = "LN0073",
    tag = "package-contains-python-hypothesis-example",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageContainsPythonHypothesisExample {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_name(ctx, NameHit::Hypothesis, Severity::Warning);
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
            files(&[("usr/share/doc/pkg/.hypothesis/other", false)]),
        ];
        let fail = vec![
            files(&[
                (
                    "usr/share/doc/python-hypothesis-example/.hypothesis/examples/show/nothing",
                    false,
                ),
                (
                    "usr/share/doc/python-hypothesis-example/.hypothesis/examples/show",
                    true,
                ),
            ]),
            files(&[("usr/lib/pkg/.hypothesis/examples/show/x", false)]).udeb(),
        ];
        BinaryPackageTester::new(PackageContainsPythonHypothesisExample, pass, fail)
            .test_and_snapshot();
    }
}
