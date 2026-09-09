use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::{DebianControl, InstallablePackageType};
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, SourceRule};

const SOURCE_FIELDS: &[&str] = &["Source", "Maintainer", "Standards-Version"];
const INSTALLABLE_FIELDS: &[&str] = &["Package", "Architecture", "Description"];
const INSTALLATION_FIELDS: &[&str] = &[
    "Package",
    "Version",
    "Architecture",
    "Maintainer",
    "Description",
];
const DSC_FIELDS: &[&str] = &[
    "Format",
    "Source",
    "Version",
    "Maintainer",
    "Standards-Version",
    "Checksums-Sha1",
    "Checksums-Sha256",
    "Files",
];

pub struct RequiredField;

declare_rule! {
    /// Policy-required Deb822 fields are missing from the Subject's primary control file.
    ///
    /// Source-tree and Source-package `debian/control` requires Source, Maintainer,
    /// and Standards-Version on the source paragraph (Standards-Version is omitted
    /// when every installable is a udeb), and Package, Architecture, and Description
    /// on each installable paragraph. Source-package Dsc also requires Format,
    /// Source, Version, Maintainer, Standards-Version, Checksums-Sha1,
    /// Checksums-Sha256, and Files (Policy 5.4); extra is the `.dsc` basename then
    /// the field. Dsc is absent on a Source tree.
    /// Binary-package Installation control requires Package, Version, Architecture,
    /// Maintainer, and Description.
    RequiredField,
    code = "LN0001",
    tag = "required-field",
    default_selected = true,
    experimental = false,
}

impl SourceRule for RequiredField {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let skip_standards_version = ctx.debian_control().is_some_and(all_udeb_installables);
        let dsc_reports = ctx.dsc().map(|dsc| {
            let basename = ctx
                .path()
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string();
            let mut fields = DSC_FIELDS.to_vec();
            if skip_standards_version {
                fields.retain(|field| *field != "Standards-Version");
            }
            fields
                .into_iter()
                .filter(|field| !dsc.fields().declares(field))
                .map(|field| format!("{basename} {field}"))
                .collect::<Vec<_>>()
        });
        if let Some(reports) = dsc_reports {
            for message in reports {
                ctx.diagnostic(Diagnostic::error(message));
            }
        }

        let Some(control) = ctx.debian_control() else {
            return;
        };

        let mut source_fields = SOURCE_FIELDS.to_vec();
        if skip_standards_version {
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

        for (field, section) in reports {
            ctx.diagnostic(
                Diagnostic::error(format!("{section} {field}")).with_location(Location {
                    path: PathBuf::from("debian/control"),
                    line: None,
                    column: None,
                }),
            );
        }
    }
}

fn all_udeb_installables(control: &DebianControl) -> bool {
    control
        .installables()
        .iter()
        .all(|installable| installable.package_type() == InstallablePackageType::Udeb)
}

impl BinaryPackageRule for RequiredField {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let reports = {
            let control = ctx.installation_control();
            let basename = ctx
                .path()
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            INSTALLATION_FIELDS
                .iter()
                .filter(|field| !control.fields().declares(field))
                .map(|field| format!("{basename} {field}"))
                .collect::<Vec<_>>()
        };
        for message in reports {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester};

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

    const COMPLETE_DSC: &str = "Format: 3.0 (native)\n\
         Source: example\n\
         Version: 1.0\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         Checksums-Sha1:\n da39a3ee5e6b4b0d3255bfef95601890afd80709 0 empty\n\
         Checksums-Sha256:\n e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 0 empty\n\
         Files:\n d41d8cd98f00b204e9800998ecf8427e 0 empty\n";

    fn dsc(text: &str) -> Dsc {
        Dsc::parse(text).expect("Dsc fixture")
    }

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
        SourceTreeTester::new(RequiredField, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let complete_control = COMPLETE.to_vec();
        let pass = vec![
            SourcePackageCase::from(complete_control.clone()).dsc(dsc(COMPLETE_DSC)),
            SourcePackageCase::from(vec![(
                "debian/control",
                "Source: example\n\
                 Maintainer: Example <ex@example.com>\n\
                 \n\
                 Package: example-udeb\n\
                 Package-Type: udeb\n\
                 Architecture: all\n\
                 Description: udeb\n extra\n",
            )])
            .dsc(dsc(
                "Format: 3.0 (native)\n\
                 Source: example\n\
                 Version: 1.0\n\
                 Maintainer: Example <ex@example.com>\n\
                 Checksums-Sha1:\n da39a3ee5e6b4b0d3255bfef95601890afd80709 0 empty\n\
                 Checksums-Sha256:\n e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 0 empty\n\
                 Files:\n d41d8cd98f00b204e9800998ecf8427e 0 empty\n",
            )),
        ];
        let fail = vec![
            SourcePackageCase::from(complete_control.clone()).dsc(dsc(
                "Format: 3.0 (native)\n\
                 Source: example\n\
                 Version: 1.0\n\
                 Maintainer: Example <ex@example.com>\n\
                 Checksums-Sha1:\n da39a3ee5e6b4b0d3255bfef95601890afd80709 0 empty\n\
                 Checksums-Sha256:\n e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 0 empty\n\
                 Files:\n d41d8cd98f00b204e9800998ecf8427e 0 empty\n",
            )),
            SourcePackageCase::from(vec![(
                "debian/control",
                "Source: example\n\
                 Maintainer: Example <ex@example.com>\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n",
            )])
            .dsc(dsc(COMPLETE_DSC)),
        ];
        SourcePackageTester::new(RequiredField, pass, fail).test_and_snapshot();
    }
}
