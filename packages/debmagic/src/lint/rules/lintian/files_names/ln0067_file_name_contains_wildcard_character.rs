use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::flag_name;
use super::names::NameHit;

pub struct FileNameContainsWildcardCharacter;

declare_rule! {
    /// An installed path contains `*` or `?`.
    ///
    /// Any Data files entry. Udebs are checked too. Extra is empty; the path
    /// is the Location.
    FileNameContainsWildcardCharacter,
    code = "LN0067",
    tag = "file-name-contains-wildcard-character",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for FileNameContainsWildcardCharacter {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_name(ctx, NameHit::Wildcard, Severity::Warning);
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
            tree(&["usr/share/foo/star", "usr/share/foo/question"]),
        ];
        let fail = vec![
            tree(&["usr/share/foo/*"]),
            tree(&[
                "usr/share/files-wildcard-characters/star*",
                "usr/share/files-wildcard-characters/question-mark?",
            ]),
            tree(&["usr/share/foo/*"]).udeb(),
        ];
        BinaryPackageTester::new(FileNameContainsWildcardCharacter, pass, fail).test_and_snapshot();
    }
}
