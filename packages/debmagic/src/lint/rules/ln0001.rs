use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::LintContext;
use crate::lint::debian_control::InstallablePackageType;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::Rule;

const SOURCE_FIELDS: &[&str] = &["Source", "Maintainer", "Standards-Version"];
const INSTALLABLE_FIELDS: &[&str] = &["Package", "Architecture", "Description"];

pub struct RequiredField;

declare_rule! {
    /// Policy-required Deb822 fields are missing from the Subject's primary control file.
    ///
    /// Source-tree `debian/control` requires Source, Maintainer, and Standards-Version
    /// on the source paragraph (Standards-Version is omitted when every installable is a
    /// udeb), and Package, Architecture, and Description on each installable paragraph.
    RequiredField,
    code = "LN0001",
    tag = "required-field",
    default_selected = true,
    experimental = false,
    applicability = [SourceTree, BinaryPackage, SourcePackage],
}

impl Rule for RequiredField {
    fn run(&self, ctx: &mut LintContext<'_>) {
        let reports = {
            let Some(control) = ctx.debian_control() else {
                return;
            };

            let mut source_fields = SOURCE_FIELDS.to_vec();
            if control
                .installables()
                .iter()
                .all(|installable| installable.package_type() == InstallablePackageType::Udeb)
            {
                source_fields.retain(|field| *field != "Standards-Version");
            }

            let mut reports = Vec::new();
            for field in source_fields {
                if !control.source().declares(field) {
                    reports.push((field.to_string(), "(in section for source)".to_string()));
                }
            }
            for installable in control.installables() {
                for field in INSTALLABLE_FIELDS {
                    if !installable.fields().declares(field) {
                        reports.push((
                            (*field).to_string(),
                            format!("(in section for {})", installable.name()),
                        ));
                    }
                }
            }
            reports
        };

        for (field, section) in reports {
            ctx.diagnostic(
                Diagnostic::error(format!("{field} {section}")).with_location(Location {
                    path: PathBuf::from("debian/control"),
                    line: None,
                    column: None,
                }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::Tester;

    use super::*;

    const COMPLETE: [(&str, &str); 1] = [(
        "debian/control",
        "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n",
    )];

    #[test]
    fn test() {
        let pass = vec![
            COMPLETE.to_vec(),
            vec![(
                "debian/control",
                "Source: example\n\
                 Maintainer: Example <ex@example.com>\n\
                 \n\
                 Package: example-udeb\n\
                 Package-Type: udeb\n\
                 Architecture: all\n\
                 Description: udeb\n extra\n",
            )],
        ];
        let fail = vec![
            vec![(
                "debian/control",
                "Source: example\n\
                 Standards-Version: 4.7.2\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n",
            )],
            vec![(
                "debian/control",
                "Source: example\n\
                 Maintainer: Example <ex@example.com>\n\
                 Standards-Version: 4.7.2\n\
                 \n\
                 Package: example\n\
                 Description: example\n extra\n",
            )],
            vec![(
                "debian/control",
                "Source: example\n\
                 Maintainer: Example <ex@example.com>\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n",
            )],
            vec![(
                "debian/control",
                "Source: example\n\
                 Maintainer: Example <ex@example.com>\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n\
                 \n\
                 Package: example-udeb\n\
                 Package-Type: udeb\n\
                 Architecture: all\n\
                 Description: udeb\n extra\n",
            )],
        ];
        Tester::new(RequiredField, pass, fail).test_and_snapshot();
    }
}
