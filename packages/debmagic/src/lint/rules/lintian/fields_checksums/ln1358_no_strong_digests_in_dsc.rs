use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::dsc::Dsc;
use crate::lint::rule::SourceRule;

pub struct NoStrongDigestsInDsc;

declare_rule! {
    /// The `.dsc` declares no `Checksum-Sha256` field, so it carries only weak
    /// digests. Reported with no extra and no Location: the Source package is
    /// the Subject. Only a Source package has a `.dsc`, so a Source tree and a
    /// Source package without one are skipped. Difference from lintian: a
    /// Source tree never reports, though lintian's oracle expects the tag for
    /// the tree too, because the oracle recipe strips `Checksums-Sha256` in a
    /// post-build step the Source tree never sees (see `tests/common/mod.rs`).
    NoStrongDigestsInDsc,
    code = "LN1358",
    tag = "no-strong-digests-in-dsc",
    default_selected = true,
    experimental = false,
}

impl SourceRule for NoStrongDigestsInDsc {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(dsc) = ctx.dsc() else {
            return;
        };
        if !declares_strong_digest(dsc) {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

fn declares_strong_digest(dsc: &Dsc) -> bool {
    dsc.fields().declares("Checksums-Sha256")
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestCase};

    use super::*;

    fn dsc(text: &str) -> Dsc {
        Dsc::parse(text).expect("Dsc fixture")
    }

    fn control() -> Vec<(&'static str, &'static str)> {
        vec![(
            "debian/control",
            "Source: example\n\
             Maintainer: Example <ex@example.com>\n\
             \n\
             Package: example\n\
             Architecture: all\n\
             Description: example\n extra\n",
        )]
    }

    #[test]
    fn source() {
        // Source tree: no `.dsc`, so nothing to report.
        let pass = vec![TestCase::from(control())];
        let fail = Vec::<TestCase>::new();
        SourceTreeTester::new(NoStrongDigestsInDsc, pass, fail).test();
    }

    #[test]
    fn source_package() {
        let strong = "Format: 3.0 (quilt)\n\
             Source: example\n\
             Checksums-Sha1:\n da39a3ee5e6b4b0d3255bfef95601890afd80709 0 example.tar.xz\n\
             Checksums-Sha256:\n e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 0 example.tar.xz\n";
        let weak = "Format: 3.0 (quilt)\n\
             Source: example\n\
             Checksums-Sha1:\n da39a3ee5e6b4b0d3255bfef95601890afd80709 0 example.tar.xz\n";
        let pass = vec![
            SourcePackageCase::from(control()).dsc(dsc(strong)),
            // Without a `.dsc` there is nothing to inspect.
            SourcePackageCase::from(control()),
        ];
        let fail = vec![SourcePackageCase::from(control()).dsc(dsc(weak))];
        SourcePackageTester::new(NoStrongDigestsInDsc, pass, fail).test_and_snapshot();
    }
}
