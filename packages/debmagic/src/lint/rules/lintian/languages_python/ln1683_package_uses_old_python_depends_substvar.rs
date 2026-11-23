use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

pub struct PackageUsesOldPythonDependsSubstvar;

declare_rule! {
    /// Some binary package of the Source package depends on the
    /// `${python:Depends}` substvar, which is for Python 2 and is obsolete
    /// since python2 left the archive. Reported at info severity, once per
    /// Installable (binary package) of the Source package, with the
    /// Installable name as the extra and no Location.
    ///
    /// As in lintian, the condition is global: when any Installable's
    /// `Pre-Depends`, `Depends`, `Recommends` or `Suggests` satisfies
    /// `${python:Depends}`, every Installable name is reported. The match is
    /// lintian's relation implication, so an alternative (`${python:Depends} |
    /// foo`) does not count.
    PackageUsesOldPythonDependsSubstvar,
    code = "LN1683",
    tag = "package-uses-old-python-depends-substvar",
    default_selected = false,
    experimental = false,
}

const RELATIONS: &[&str] = &["Pre-Depends", "Depends", "Recommends", "Suggests"];

impl SourceRule for PackageUsesOldPythonDependsSubstvar {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let installables = control.installables();
        let uses_substvar = installables.iter().any(|installable| {
            installable
                .fields()
                .relations_and(RELATIONS)
                .satisfies("${python:Depends}")
        });
        if !uses_substvar {
            return;
        }
        let names: Vec<String> = installables
            .iter()
            .map(|installable| installable.name().to_string())
            .collect();
        for name in names {
            ctx.diagnostic(Diagnostic::warn(name).with_severity(Severity::Info));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(binary: &str) -> String {
        format!(
            "Source: example\n\
             Maintainer: Example <ex@example.com>\n\
             Standards-Version: 4.7.2\n\
             \n\
             Package: example\n\
             Architecture: all\n\
             {binary}\
             Description: example\n extra\n"
        )
    }

    fn tree(binary: &str) -> Vec<(String, TestFile)> {
        vec![(
            "debian/control".to_string(),
            TestFile::from(control(binary)),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(""),
            tree("Depends: ${misc:Depends}, ${python3:Depends}\n"),
            // An alternative does not imply the substvar.
            tree("Depends: ${python:Depends} | foo\n"),
            // Build-Depends are not binary relations.
            tree("Depends: foo\n"),
        ];
        let fail = vec![
            tree("Depends: ${misc:Depends}, ${python:Depends}\n"),
            tree("Recommends: ${python:Depends}\n"),
            tree("Pre-Depends: ${python:Depends}\n"),
        ];
        SourceTreeTester::new(PackageUsesOldPythonDependsSubstvar, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            "Depends: ${python3:Depends}\n",
        ))];
        let fail = vec![SourcePackageCase::from(tree(
            "Depends: ${python:Depends}\n",
        ))];
        SourcePackageTester::new(PackageUsesOldPythonDependsSubstvar, pass, fail)
            .test_and_snapshot();
    }
}
