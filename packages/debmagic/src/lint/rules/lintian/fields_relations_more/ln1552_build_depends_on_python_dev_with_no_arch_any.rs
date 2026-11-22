use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::overlap::build_depends_all;

/// lintian's `$PYTHON_DEV`: the python development packages that are almost
/// always only needed to build architecture-dependent modules.
const PYTHON_DEV: &str = "python3-dev | python3-all-dev | python2.7-dev:any | python3-dev:any | python3.7-dev:any | python3.8-dev:any | python3.9-dev:any";

pub struct BuildDependsOnPythonDevWithNoArchAny;

declare_rule! {
    /// The source package builds only `Architecture: all` binary packages but
    /// build-depends on a python development package.
    ///
    /// Lintian's `fields/package-relations` Check, on the source paragraph.
    /// Reported when every installable of `debian/control` is
    /// `Architecture: all` (an installable without an Architecture counts as
    /// architecture-dependent) and the union of `Build-Depends`,
    /// `Build-Depends-Indep` and `Build-Depends-Arch` implies one of the
    /// `python*-dev` packages of lintian's `$PYTHON_DEV`. Extra is empty.
    /// Location is empty. Applies to the Source tree and the Source package.
    BuildDependsOnPythonDevWithNoArchAny,
    code = "LN1552",
    tag = "build-depends-on-python-dev-with-no-arch-any",
    default_selected = false,
    experimental = false,
}

impl SourceRule for BuildDependsOnPythonDevWithNoArchAny {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let report = match ctx.debian_control() {
            Some(control) => {
                let arch_dependent = control
                    .installables()
                    .iter()
                    .filter(|installable| installable.fields().get("Architecture") != Some("all"))
                    .count();
                arch_dependent == 0 && build_depends_all(control.source()).satisfies(PYTHON_DEV)
            }
            None => false,
        };
        if report {
            ctx.diagnostic(Diagnostic::warn("").with_severity(Severity::Info));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn control(source: &str, binary_arch: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: Example <ex@example.com>\n{source}\n\
                 \nPackage: example\nArchitecture: {binary_arch}\nDescription: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            control("Build-Depends: debhelper", "all"),
            control("Build-Depends: python3-dev", "any"),
            control("Build-Depends: python3-dev | foo", "all"),
            control("Build-Depends: foo", "all"),
            vec![("README", TestFile::new("no packaging"))],
        ];
        let fail = vec![
            control("Build-Depends: python3-dev", "all"),
            control("Build-Depends-Indep: python3-all-dev", "all"),
            control("Build-Depends-Arch: python2.7-dev:any", "all"),
            control("Build-Depends: python3.9-dev:any", "all"),
            control("Build-Depends: python3-dev:any", "all"),
        ];
        SourceTreeTester::new(BuildDependsOnPythonDevWithNoArchAny, pass, fail).test_and_snapshot();
    }
}
