use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

pub struct RedundantPriorityOptionalField;

declare_rule! {
    /// `Priority: optional` is redundant in `debian/control`.
    ///
    /// Since dpkg 1.22.13 `optional` is the default, so the field can be removed.
    /// The source paragraph's `Priority` is reported when its value is `optional`.
    /// An installable's `Priority` is checked only when the source paragraph does
    /// not declare `Priority` or declares it as `optional`; it is reported when its
    /// value is `optional`. The extra is empty and the Location is `debian/control`
    /// at the line of the field name. Applies to the Source tree and Source package.
    RedundantPriorityOptionalField,
    code = "LN1314",
    tag = "redundant-priority-optional-field",
    default_selected = false,
    experimental = false,
}

const OPTIONAL: &str = "optional";

impl SourceRule for RedundantPriorityOptionalField {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let source = control.source();
        let mut lines = Vec::new();
        if source.get("Priority") == Some(OPTIONAL) {
            lines.push(source.position("Priority"));
        }
        if !source.declares("Priority") || source.get("Priority") == Some(OPTIONAL) {
            for installable in control.installables() {
                let fields = installable.fields();
                if fields.get("Priority") == Some(OPTIONAL) {
                    lines.push(fields.position("Priority"));
                }
            }
        }
        for line in lines {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path: PathBuf::from("debian/control"),
                        line,
                        column: None,
                    }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(source_fields: &str, installable_fields: &str) -> String {
        format!(
            "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
{source_fields}\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n\
{installable_fields}"
        )
    }

    fn package(source_fields: &str, installable_fields: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(control(source_fields, installable_fields)),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no control"))],
            package("", ""),
            package("Priority: extra\n", ""),
            // Installable Priority is not checked when the source declares another value.
            package("Priority: extra\n", "Priority: optional\n"),
            package("", "Priority: extra\n"),
        ];
        let fail = vec![
            package("Priority: optional\n", ""),
            package("", "Priority: optional\n"),
            package("Priority: optional\n", "Priority: optional\n"),
            // A udeb installable is checked the same way.
            package("", "Package-Type: udeb\nPriority: optional\n"),
        ];
        SourceTreeTester::new(RedundantPriorityOptionalField, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package("Priority: extra\n", ""))];
        let fail = vec![SourcePackageCase::from(package("Priority: optional\n", ""))];
        SourcePackageTester::new(RedundantPriorityOptionalField, pass, fail).test_and_snapshot();
    }
}
