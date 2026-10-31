use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::flag_name;
use super::names::NameHit;

pub struct StarFile;

declare_rule! {
    /// A path's final component is a literal `*`, after a slash.
    ///
    /// `*` at the package root is not flagged. A directory whose lintian name
    /// ends in `/*/` is not this Tag (the slash is part of the directory name).
    /// Udebs are checked too. Extra is empty; the path is the Location.
    StarFile,
    code = "LN0079",
    tag = "star-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for StarFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_name(ctx, NameHit::Star, Severity::Error);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            BinaryPackageCase::from(vec![
                ("usr/share/foo/star".to_string(), TestFile::new("x")),
                ("*".to_string(), TestFile::new("x")),
                // A directory name gains a trailing slash, so it does not end in `/*`.
                ("usr/bin/*".to_string(), TestFile::directory()),
            ]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/share/foo/*".to_string(), TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("usr/share/foo/*".to_string(), TestFile::new("x"))])
                .udeb(),
        ];
        BinaryPackageTester::new(StarFile, pass, fail).test_and_snapshot();
    }
}
