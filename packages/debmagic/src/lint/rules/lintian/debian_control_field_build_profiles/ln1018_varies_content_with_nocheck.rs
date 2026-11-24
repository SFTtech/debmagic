use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::ln1017_invalid_profile_name_in_build_profiles_field::{
    build_profiles_location, restrictions,
};

pub struct VariesContentWithNocheck;

declare_rule! {
    /// A `Build-Profiles` restriction formula uses the `nocheck` profile.
    ///
    /// Checks the same formulas as `invalid-profile-name-in-build-profiles-field`
    /// (`debian/control`, every installable, skipped when the value is not a
    /// restriction formula). Each `nocheck` profile, with or without a leading
    /// `!`, is one finding. Extra is empty, so the Location (the `Build-Profiles`
    /// line of `debian/control`) is part of what is compared.
    VariesContentWithNocheck,
    code = "LN1018",
    tag = "varies-content-with-nocheck",
    default_selected = true,
    experimental = false,
}

impl SourceRule for VariesContentWithNocheck {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for restriction in restrictions(ctx) {
            let nochecks = restriction
                .profiles
                .iter()
                .filter(|profile| profile.as_str() == "nocheck");
            for _ in nochecks {
                ctx.diagnostic(
                    Diagnostic::error("").with_location(build_profiles_location(&restriction)),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const SOURCE: &str = "Source: example\nMaintainer: Example <ex@example.com>\n\n";

    fn control(build_profiles: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "{SOURCE}Package: example\nArchitecture: all\n\
                 Build-Profiles: {build_profiles}\nDescription: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn source() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            control("<cross> <nodoc>"),
            control("<!noinsttest> <stage1>"),
            control("<nocheck"),
            control("<nochecks>"),
            vec![(
                "debian/control",
                TestFile::new(format!("{SOURCE}Package: example\nArchitecture: all\n")),
            )],
        ];
        let fail = vec![
            control("<nocheck>"),
            control("<!nocheck> <cross>"),
            control("<stage1 nocheck> <!nocheck noinsttest>"),
        ];
        SourceTreeTester::new(VariesContentWithNocheck, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let dsc = || crate::lint::dsc::Dsc::parse("Source: example\n").expect("Dsc");
        let pass = vec![SourcePackageCase::from(control("<cross>")).dsc(dsc())];
        let fail = vec![SourcePackageCase::from(control("<nocheck>")).dsc(dsc())];
        SourcePackageTester::new(VariesContentWithNocheck, pass, fail).test_and_snapshot();
    }
}
