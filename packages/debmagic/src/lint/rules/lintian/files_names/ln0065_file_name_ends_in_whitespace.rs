use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::flag_name;
use super::names::NameHit;

pub struct FileNameEndsInWhitespace;

declare_rule! {
    /// An installed path ends in whitespace.
    ///
    /// Any Data files entry whose path ends in a Unicode whitespace character.
    /// Udebs are checked too. Extra is empty; the path is the Location.
    FileNameEndsInWhitespace,
    code = "LN0065",
    tag = "file-name-ends-in-whitespace",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for FileNameEndsInWhitespace {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_name(ctx, NameHit::Whitespace, Severity::Warning);
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
            tree(&["usr/share/foo/ws"]),
            tree(&["usr/share/foo/ws.txt"]),
        ];
        let fail = vec![
            tree(&["usr/share/foo/ws "]),
            tree(&["files/'\\ "]),
            tree(&["usr/share/foo/tab\t"]).udeb(),
        ];
        BinaryPackageTester::new(FileNameEndsInWhitespace, pass, fail).test_and_snapshot();
    }
}
