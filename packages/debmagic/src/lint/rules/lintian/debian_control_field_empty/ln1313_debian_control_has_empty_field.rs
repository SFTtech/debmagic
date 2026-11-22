use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

pub struct DebianControlHasEmptyField;

declare_rule! {
    /// A field of a `debian/control` paragraph has no value.
    ///
    /// Policy permits an empty field, but lintian's Check ignores it, so it is
    /// reported for removal. Every field name of the source paragraph and of each
    /// installable paragraph whose value is empty is reported. The extra is
    /// `(in source paragraph)` or `(in section for <installable>)`, then the field
    /// name as written. The Location is `debian/control` at the field's line.
    /// Applies to the Source tree and Source package.
    DebianControlHasEmptyField,
    code = "LN1313",
    tag = "debian-control-has-empty-field",
    default_selected = false,
    experimental = false,
}

impl SourceRule for DebianControlHasEmptyField {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let mut reports = Vec::new();
        collect(control.source(), "(in source paragraph)", &mut reports);
        for installable in control.installables() {
            let section = format!("(in section for {})", installable.name());
            collect(installable.fields(), &section, &mut reports);
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

/// Reports each field of one paragraph whose value is empty.
fn collect(paragraph: &ControlParagraph, section: &str, reports: &mut Vec<(Option<u32>, String)>) {
    for field in paragraph.names() {
        if paragraph.get(field).is_some_and(|value| value.is_empty()) {
            reports.push((paragraph.position(field), format!("{section} {field}")));
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
            package("Vcs-Git: https://example.org/repo.git\n", "Suggests: foo\n"),
        ];
        let fail = vec![
            package("Vcs-Git:\n", ""),
            package("Vcs-Git:    \n", ""),
            package("", "Suggests:\n"),
            package("", "Provides:\nSection: misc\n"),
        ];
        SourceTreeTester::new(DebianControlHasEmptyField, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package("Vcs-Git: x\n", ""))];
        let fail = vec![SourcePackageCase::from(package("Vcs-Git:\n", ""))];
        SourcePackageTester::new(DebianControlHasEmptyField, pass, fail).test_and_snapshot();
    }
}
