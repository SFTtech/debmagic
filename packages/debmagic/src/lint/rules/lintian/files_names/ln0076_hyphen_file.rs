use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::flag::flag_name;
use super::names::NameHit;
use crate::lint::rule::Severity;

pub struct HyphenFile;

declare_rule! {
    /// An installed path's final component is a single hyphen.
    ///
    /// The path must contain a slash before that hyphen (`/-` at the end).
    /// A file named `-` at the package root is not flagged. Udebs are
    /// checked too. Extra is empty; the path is the Location.
    HyphenFile,
    code = "LN0076",
    tag = "hyphen-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for HyphenFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_name(ctx, NameHit::Hyphen, Severity::Error);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::new("x")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            tree(&["usr/share/foo/a-b", "usr/share/foo/-/nested", "-"]),
        ];
        let fail = vec![tree(&["usr/share/foo/-"]), tree(&["usr/bin/-"]).udeb()];
        BinaryPackageTester::new(HyphenFile, pass, fail).test_and_snapshot();
    }
}
