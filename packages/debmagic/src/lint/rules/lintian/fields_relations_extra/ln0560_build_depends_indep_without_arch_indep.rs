use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::DebianControl;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::is_arch_all;

pub struct BuildDependsIndepWithoutArchIndep;

declare_rule! {
    /// `Build-Depends-Indep` is declared and no installable is `Architecture: all`.
    ///
    /// Counts the binary paragraphs of `debian/control` whose `Architecture`
    /// is exactly `all`; a missing `Architecture` counts as architecture
    /// dependent. Extra and Location are empty.
    BuildDependsIndepWithoutArchIndep,
    code = "LN0560",
    tag = "build-depends-indep-without-arch-indep",
    default_selected = true,
    experimental = false,
}

impl SourceRule for BuildDependsIndepWithoutArchIndep {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let hit = ctx.debian_control().is_some_and(applies);
        if hit {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

fn applies(control: &DebianControl) -> bool {
    control.source().declares("Build-Depends-Indep")
        && !control
            .installables()
            .iter()
            .any(|installable| is_arch_all(installable.fields()))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::super::testing::raw;
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            raw(
                "Source: example\nBuild-Depends: debhelper\n\nPackage: example\nArchitecture: any\n",
            ),
            raw("Source: example\nBuild-Depends-Indep: doxygen\n\n\
                 Package: example\nArchitecture: any\n\n\
                 Package: example-doc\nArchitecture: all\n"),
        ];
        let fail = vec![
            raw(
                "Source: example\nBuild-Depends-Indep: doxygen\n\nPackage: example\nArchitecture: any\n",
            ),
            // A missing Architecture is not "all".
            raw("Source: example\nBuild-Depends-Indep: doxygen\n\nPackage: example\n"),
            raw("Source: example\nBuild-Depends-Indep: doxygen\n"),
        ];
        SourceTreeTester::new(BuildDependsIndepWithoutArchIndep, pass, fail).test_and_snapshot();
    }
}
