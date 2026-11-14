use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::DebianControl;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::is_arch_all;

pub struct BuildDependsArchWithoutArchDependentBinary;

declare_rule! {
    /// `Build-Depends-Arch` is declared and every installable is `Architecture: all`.
    ///
    /// Counts the binary paragraphs of `debian/control` whose `Architecture`
    /// is not exactly `all`; a missing `Architecture` counts as architecture
    /// dependent. Extra and Location are empty.
    BuildDependsArchWithoutArchDependentBinary,
    code = "LN0561",
    tag = "build-depends-arch-without-arch-dependent-binary",
    default_selected = true,
    experimental = false,
}

impl SourceRule for BuildDependsArchWithoutArchDependentBinary {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let hit = ctx.debian_control().is_some_and(applies);
        if hit {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

fn applies(control: &DebianControl) -> bool {
    control.source().declares("Build-Depends-Arch")
        && control
            .installables()
            .iter()
            .all(|installable| is_arch_all(installable.fields()))
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
                "Source: example\nBuild-Depends: debhelper\n\nPackage: example\nArchitecture: all\n",
            ),
            raw(
                "Source: example\nBuild-Depends-Arch: gcc\n\nPackage: example\nArchitecture: any\n",
            ),
            raw("Source: example\nBuild-Depends-Arch: gcc\n\n\
                 Package: example\nArchitecture: all\n\n\
                 Package: example-bin\nArchitecture: amd64\n"),
            // A missing Architecture is not "all".
            raw("Source: example\nBuild-Depends-Arch: gcc\n\nPackage: example\n"),
        ];
        let fail = vec![
            raw(
                "Source: example\nBuild-Depends-Arch: gcc\n\nPackage: example\nArchitecture: all\n",
            ),
            // No installable at all: none is arch-dependent.
            raw("Source: example\nBuild-Depends-Arch: gcc\n"),
        ];
        SourceTreeTester::new(BuildDependsArchWithoutArchDependentBinary, pass, fail)
            .test_and_snapshot();
    }
}
