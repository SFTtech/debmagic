use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct CircularInstallationPrerequisite;

declare_rule! {
    /// An installable's installation prerequisite names the installable itself.
    ///
    /// `Pre-Depends`, `Depends`, `Recommends`, and `Suggests` of each
    /// `debian/control` installable, in that order. The field must imply the
    /// installable's name: a versioned constraint still does, an alternative
    /// does not unless every side does, and an architecture restriction does
    /// not. Extra is `(in section for <package>)`, the field, and the
    /// relation in lintian's spacing. Applies to the Source tree and Source
    /// package.
    CircularInstallationPrerequisite,
    code = "LN0450",
    tag = "circular-installation-prerequisite",
    default_selected = true,
    experimental = false,
}

const FIELDS: &[&str] = &["Pre-Depends", "Depends", "Recommends", "Suggests"];

impl SourceRule for CircularInstallationPrerequisite {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let mut messages = Vec::new();
        for installable in control.installables() {
            for field in FIELDS {
                let Some(relation) = installable.fields().relation(field) else {
                    continue;
                };
                if relation.satisfies(installable.name()) {
                    messages.push(format!(
                        "(in section for {}) {field} {}",
                        installable.name(),
                        relation.to_lintian_string()
                    ));
                }
            }
        }
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/control", TestFile::new(body))]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            control(
                "\
Source: example\n\
\n\
Package: example\n\
Architecture: all\n\
Depends: other\n\
Description: example\n extra\n",
            ),
            control(
                "\
Source: example\n\
\n\
Package: example\n\
Architecture: all\n\
Depends: example | other\n\
Description: example\n extra\n",
            ),
            control(
                "\
Source: example\n\
\n\
Package: example\n\
Architecture: all\n\
Depends: example [amd64]\n\
Conflicts: example\n\
Description: example\n extra\n",
            ),
        ];
        let fail = vec![
            control(
                "\
Source: example\n\
\n\
Package: example\n\
Architecture: all\n\
Suggests: example\n\
Description: example\n extra\n",
            ),
            control(
                "\
Source: example\n\
\n\
Package: example\n\
Architecture: all\n\
Depends: other, example (>= 1)\n\
Recommends: example | example\n\
Description: example\n extra\n",
            ),
            control(
                "\
Source: example\n\
\n\
Package: first\n\
Architecture: all\n\
Pre-Depends: first\n\
Description: first\n extra\n\
\n\
Package: second\n\
Architecture: all\n\
Depends: bash (>> 2.0 ), second, ${misc:Depends}\n\
Description: second\n extra\n",
            ),
        ];
        SourceTreeTester::new(CircularInstallationPrerequisite, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(control(
            "\
Source: example\n\
\n\
Package: example\n\
Architecture: all\n\
Depends: other\n\
Description: example\n extra\n",
        ))];
        let fail = vec![SourcePackageCase::from(control(
            "\
Source: example\n\
\n\
Package: example\n\
Architecture: all\n\
Depends: example\n\
Description: example\n extra\n",
        ))];
        SourcePackageTester::new(CircularInstallationPrerequisite, pass, fail).test_and_snapshot();
    }
}
