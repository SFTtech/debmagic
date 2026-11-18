use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::{
    is_newer_than_latest_policy, is_policy_release, is_three_part_number, standards_version,
};

pub struct InvalidStandardsVersion;

declare_rule! {
    /// The Standards-Version is not a Policy release.
    ///
    /// Fires when the Standards-Version is not three or more dot-separated
    /// numbers, or when it is not a known Policy release and does not sort
    /// after the newest one (a newer one is `newer-standards-version`).
    /// Trailing dots are dropped before the count, as Perl's `split` does.
    /// Extra is the Standards-Version as written. Applies to the Source tree
    /// and Source package.
    InvalidStandardsVersion,
    code = "LN0774",
    tag = "invalid-standards-version",
    default_selected = true,
    experimental = false,
}

impl SourceRule for InvalidStandardsVersion {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(version) = standards_version(ctx) else {
            return;
        };
        let invalid = !is_three_part_number(&version)
            || (!is_policy_release(&version) && !is_newer_than_latest_policy(&version));
        if invalid {
            ctx.diagnostic(Diagnostic::error(version));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(standards_version: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nStandards-Version: {standards_version}\n\n\
                 Package: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            control("4.7.2"),
            control("4.7.4.1"),
            // Known, with and without the fourth component.
            control("3.9.6"),
            control("4.7.2.0"),
            // Not known, but newer: `newer-standards-version`.
            control("4.7.99"),
            control("5.15.0.0"),
        ];
        let fail = vec![
            // Fewer than three numbers, or not numbers.
            control("3.8"),
            control("4"),
            control("4.x.2"),
            control("4..2"),
            control(""),
            // Not a known release, and older than the newest one.
            control("1.2.3"),
            control("4.7.3.1"),
            // Trailing dots are dropped for the count; the rest is not known.
            control("4.7.2."),
        ];
        SourceTreeTester::new(InvalidStandardsVersion, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let dsc = |standards_version: &str| {
            Dsc::parse(&format!(
                "Source: example\nStandards-Version: {standards_version}\n"
            ))
            .expect("Dsc")
        };
        let pass = vec![
            SourcePackageCase::from(Vec::<(&str, TestFile)>::new()).dsc(dsc("4.7.2")),
            // The Dsc decides: debian/control's 3.8 is not read.
            SourcePackageCase::from(control("3.8")).dsc(dsc("4.7.2")),
        ];
        let fail = vec![
            SourcePackageCase::from(Vec::<(&str, TestFile)>::new()).dsc(dsc("3.8")),
            // The Dsc decides: debian/control's 4.7.2 is not read.
            SourcePackageCase::from(control("4.7.2")).dsc(dsc("1.2.3")),
        ];
        SourcePackageTester::new(InvalidStandardsVersion, pass, fail).test_and_snapshot();
    }
}
