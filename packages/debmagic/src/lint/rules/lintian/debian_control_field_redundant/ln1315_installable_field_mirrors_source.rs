use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

pub struct InstallableFieldMirrorsSource;

declare_rule! {
    /// An installable paragraph repeats a source field with the same value.
    ///
    /// Policy lets an installable paragraph inherit fields from the source
    /// paragraph, so a field that repeats the source's value is redundant. A field
    /// is reported when the source paragraph declares it and the two values are
    /// equal. The extra is `(in section for <installable>)`, then the field name
    /// as written. The Location is `debian/control` at the field's line. Applies to
    /// the Source tree and Source package.
    InstallableFieldMirrorsSource,
    code = "LN1315",
    tag = "installable-field-mirrors-source",
    default_selected = false,
    experimental = false,
}

impl SourceRule for InstallableFieldMirrorsSource {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let source = control.source();
        let mut reports = Vec::new();
        for installable in control.installables() {
            let fields = installable.fields();
            let section = format!("(in section for {})", installable.name());
            for field in fields.names() {
                if source.declares(field) && fields.get(field) == source.get(field) {
                    reports.push((fields.position(field), format!("{section} {field}")));
                }
            }
        }
        for (line, extra) in reports {
            ctx.diagnostic(
                Diagnostic::warn(extra)
                    .with_severity(Severity::Info)
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
            // Same name, different value.
            package("Section: misc\n", "Section: libs\n"),
            // Value on the source only, or declared on the installable only.
            package("Section: misc\n", "Priority: optional\n"),
            package("", "Section: misc\n"),
        ];
        let fail = vec![
            // Same name and same value on both paragraphs.
            package("Section: misc\n", "Section: misc\n"),
            package(
                "Maintainer: Example <ex@example.com>\n",
                "Maintainer: Example <ex@example.com>\n",
            ),
            // Udeb installables are checked too.
            package("Section: misc\n", "Package-Type: udeb\nSection: misc\n"),
        ];
        SourceTreeTester::new(InstallableFieldMirrorsSource, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(
            "Section: misc\n",
            "Section: libs\n",
        ))];
        let fail = vec![SourcePackageCase::from(package(
            "Section: misc\n",
            "Section: misc\n",
        ))];
        SourcePackageTester::new(InstallableFieldMirrorsSource, pass, fail).test_and_snapshot();
    }
}
