use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct LibmoduleBuildTinyPerlNeedsToBeInBuildDepends;

declare_rule! {
    /// `libmodule-build-tiny-perl` is implied by `Build-Depends-Indep` but not by `Build-Depends`.
    ///
    /// The package is used by the clean target, so it belongs in
    /// `Build-Depends`. Uses the relation implication of `satisfies`, so a
    /// versioned or alternative entry in `Build-Depends-Indep` counts. Applies
    /// to source trees and source packages.
    LibmoduleBuildTinyPerlNeedsToBeInBuildDepends,
    code = "LN0938",
    tag = "libmodule-build-tiny-perl-needs-to-be-in-build-depends",
    default_selected = true,
    experimental = false,
}

impl SourceRule for LibmoduleBuildTinyPerlNeedsToBeInBuildDepends {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let misplaced = ctx.debian_control().is_some_and(|control| {
            let source = control.source();
            let indep = source.relation("Build-Depends-Indep").unwrap_or_default();
            let depends = source.relation("Build-Depends").unwrap_or_default();
            indep.satisfies(MODULE) && !depends.satisfies(MODULE)
        });
        if misplaced {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

const MODULE: &str = "libmodule-build-tiny-perl";

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn control(fields: &[(&str, &str)]) -> Vec<(&'static str, TestFile)> {
        let mut body = String::from("Source: example\n");
        for (name, value) in fields {
            body.push_str(name);
            body.push_str(": ");
            body.push_str(value);
            body.push('\n');
        }
        body.push_str("\nPackage: example\nArchitecture: all\nDescription: example\n extra\n");
        vec![("debian/control", TestFile::new(body))]
    }

    #[test]
    fn test() {
        let pass = vec![
            control(&[]),
            control(&[("Build-Depends", "libmodule-build-tiny-perl, debhelper")]),
            control(&[
                ("Build-Depends", "libmodule-build-tiny-perl (>= 0.1)"),
                ("Build-Depends-Indep", "libmodule-build-tiny-perl"),
            ]),
            control(&[("Build-Depends-Indep", "libfoo-perl")]),
            control(&[("Build-Depends-Indep", "libmodule-build-tiny-perl-extra")]),
        ];
        let fail = vec![
            control(&[("Build-Depends-Indep", "libmodule-build-tiny-perl")]),
            control(&[
                ("Build-Depends", "debhelper"),
                ("Build-Depends-Indep", "libmodule-build-tiny-perl (>= 0.2)"),
            ]),
        ];
        SourceTreeTester::new(LibmoduleBuildTinyPerlNeedsToBeInBuildDepends, pass, fail)
            .test_and_snapshot();
    }
}
