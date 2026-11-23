use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

pub struct ManualPageInUdeb;

declare_rule! {
    /// A udeb ships a manual page. Lintian 2.144.0 never reports this.
    ///
    /// The lintian Check `documentation/manual` returns before it walks the
    /// installed files of a udeb, and the `manual-page-in-udeb` hint sits
    /// inside that walk, guarded by the same udeb test. No path reaches the
    /// hint, so the Tag is never emitted. This Rule follows that behaviour and
    /// reports nothing for any Binary package. It stays registered so the Tag
    /// is catalogued. Extra and Location would be empty and the Subject the
    /// package.
    ManualPageInUdeb,
    code = "LN1776",
    tag = "manual-page-in-udeb",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ManualPageInUdeb {
    fn run(&self, _ctx: &mut BinaryPackageContext<'_>) {}
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn with_man_page(udeb: bool) -> BinaryPackageCase {
        let case = BinaryPackageCase::from(vec![
            (
                "usr/share/man/man1/foo.1.gz".to_string(),
                TestFile::new("x"),
            ),
            ("usr/bin/foo".to_string(), TestFile::new("x")),
        ]);
        if udeb { case.udeb() } else { case }
    }

    #[test]
    fn binary() {
        // Lintian never emits this Tag, so udebs and debs with man pages pass.
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            with_man_page(true),
            with_man_page(false),
        ];
        let fail = Vec::<BinaryPackageCase>::new();
        BinaryPackageTester::new(ManualPageInUdeb, pass, fail).test();
    }
}
