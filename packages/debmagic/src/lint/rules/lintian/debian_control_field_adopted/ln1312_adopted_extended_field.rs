use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};
use crate::lint::rules::lintian::known_fields::{binary_resembles, source_resembles};

pub struct AdoptedExtendedField;

declare_rule! {
    /// A field in `debian/control` carries an extension prefix (`X…-`) but its bare
    /// name is already a known field.
    ///
    /// Each field of the source paragraph whose name starts with `X` and whose text
    /// after the first `-` resembles a name in lintian's `common/source-fields` is
    /// reported; each installable paragraph is checked the same way against
    /// `fields/binary-fields`. The name match is case-insensitive and uses the raw
    /// table entries, so `XS-Vcs-Svn` is reported as `Vcs-Svn` is known. The extra is
    /// `(in section for source)` or `(in section for <installable>)`, then the field
    /// name as written. The Location is `debian/control` at the field's line.
    /// Applies to the Source tree and Source package.
    AdoptedExtendedField,
    code = "LN1312",
    tag = "adopted-extended-field",
    default_selected = false,
    experimental = false,
}

impl SourceRule for AdoptedExtendedField {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let mut reports: Vec<(Option<u32>, String)> = Vec::new();
        collect(
            control.source(),
            "(in section for source)",
            source_resembles,
            &mut reports,
        );
        for installable in control.installables() {
            let section = format!("(in section for {})", installable.name());
            collect(
                installable.fields(),
                &section,
                binary_resembles,
                &mut reports,
            );
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

/// Reports each `X…-Bare` field of one paragraph whose `Bare` resembles a known name.
fn collect(
    paragraph: &ControlParagraph,
    section: &str,
    resembles: fn(&str) -> bool,
    reports: &mut Vec<(Option<u32>, String)>,
) {
    for field in paragraph.names() {
        let Some((marker, bare)) = field.split_once('-') else {
            continue;
        };
        if marker.is_empty() || bare.is_empty() {
            continue;
        }
        if !marker.starts_with(['X', 'x']) || !resembles(bare) {
            continue;
        }
        reports.push((paragraph.position(field), format!("{section} {field}")));
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
            // Unprefixed names are not this Tag.
            package("Vcs-Svn: svn://example\n", ""),
            // A prefix on a name that is not known is not this Tag.
            package("XS-Unknown-Thing: yes\n", ""),
            // Marker without X is not this Tag.
            package("S-Vcs-Svn: yes\n", ""),
            // Empty marker or bare name is skipped.
            package("-Vcs-Svn: yes\n", ""),
            package("XS-: yes\n", ""),
        ];
        let fail = vec![
            package("XS-Vcs-Svn: svn://example\n", ""),
            package("XS-Vcs-Svn: a\nXS-Testsuite: autopkgtest\n", ""),
            // Case-insensitive on the prefix and the bare name.
            package("xs-vcs-svn: svn://example\n", ""),
            // Installable paragraphs use the binary table.
            package("", "XC-Package-Type: udeb\n"),
        ];
        SourceTreeTester::new(AdoptedExtendedField, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package("Vcs-Svn: x\n", ""))];
        let fail = vec![SourcePackageCase::from(package("XS-Vcs-Svn: x\n", ""))];
        SourcePackageTester::new(AdoptedExtendedField, pass, fail).test_and_snapshot();
    }
}
