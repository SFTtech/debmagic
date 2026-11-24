use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::package_relation::PackageRelation;
use crate::lint::rule::SourceRule;

pub struct LibmoduleBuildPerlNeedsToBeInBuildDepends;

declare_rule! {
    /// `libmodule-build-perl` is in `Build-Depends-Indep` but not in `Build-Depends`.
    ///
    /// The package is used by the clean target, so it belongs in
    /// `Build-Depends`. Matches the package name of any alternative in either
    /// field. Applies to source trees and source packages.
    LibmoduleBuildPerlNeedsToBeInBuildDepends,
    code = "LN0937",
    tag = "libmodule-build-perl-needs-to-be-in-build-depends",
    default_selected = true,
    experimental = false,
}

impl SourceRule for LibmoduleBuildPerlNeedsToBeInBuildDepends {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let misplaced = ctx.debian_control().is_some_and(|control| {
            let source = control.source();
            let indep = source.relation("Build-Depends-Indep").unwrap_or_default();
            let depends = source.relation("Build-Depends").unwrap_or_default();
            names(&indep, MODULE) && !names(&depends, MODULE)
        });
        if misplaced {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

const MODULE: &str = "libmodule-build-perl";

fn names(relation: &PackageRelation, name: &str) -> bool {
    relation
        .alternatives()
        .any(|alternative| alternative.package() == name)
}

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
            control(&[("Build-Depends", "libmodule-build-perl, debhelper")]),
            control(&[
                ("Build-Depends", "libmodule-build-perl"),
                ("Build-Depends-Indep", "libmodule-build-perl"),
            ]),
            control(&[("Build-Depends-Indep", "libfoo-perl")]),
            control(&[("Build-Depends-Indep", "libmodule-build-perl-extra")]),
        ];
        let fail = vec![
            control(&[("Build-Depends-Indep", "libmodule-build-perl")]),
            control(&[
                ("Build-Depends", "debhelper"),
                ("Build-Depends-Indep", "libmodule-build-perl | libfoo"),
            ]),
        ];
        SourceTreeTester::new(LibmoduleBuildPerlNeedsToBeInBuildDepends, pass, fail)
            .test_and_snapshot();
    }
}
