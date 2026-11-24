use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct PackageDependsOnHardcodedLibc;

declare_rule! {
    /// An installable's `Pre-Depends`, `Depends`, `Recommends` or `Suggests`
    /// in `debian/control` satisfies `libc6 | libc6.1 | libc0.1 | libc0.3`:
    /// the package depends on libc directly instead of through
    /// `${shlibs:Depends}`.
    ///
    /// Skipped when the source package is `glibc` or `eglibc`. Each field is
    /// read per installable, in the order `Pre-Depends`, `Depends`,
    /// `Recommends`, `Suggests`. Extra is `(in section for <installable>)
    /// <field> <relation>`, the relation in lintian's standard spacing, and
    /// Location is `debian/control` with the line of the field (no line when
    /// the raw file has no paragraph naming the installable).
    PackageDependsOnHardcodedLibc,
    code = "LN1135",
    tag = "package-depends-on-hardcoded-libc",
    default_selected = true,
    experimental = false,
}

const LIBC_RELATION: &str = "libc6 | libc6.1 | libc0.1 | libc0.3";
const PREREQUISITE_FIELDS: &[&str] = &["Pre-Depends", "Depends", "Recommends", "Suggests"];

impl SourceRule for PackageDependsOnHardcodedLibc {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        if matches!(control.source().get("Source"), Some("glibc" | "eglibc")) {
            return;
        }

        let mut reports = Vec::new();
        for installable in control.installables() {
            let fields = installable.fields();
            for field in PREREQUISITE_FIELDS {
                if !fields.declares(field) {
                    continue;
                }
                let Some(relation) = fields.relation(field) else {
                    continue;
                };
                if !relation.satisfies(LIBC_RELATION) {
                    continue;
                }
                reports.push((
                    format!(
                        "(in section for {}) {} {}",
                        installable.name(),
                        field,
                        relation.to_lintian_string()
                    ),
                    fields.position(field),
                ));
            }
        }

        for (extra, line) in reports {
            ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
                path: PathBuf::from("debian/control"),
                line,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester};

    use super::*;

    const DSC: &str = "Format: 3.0 (native)\n\
         Source: example\n\
         Version: 1.0\n\
         Maintainer: Example <ex@example.com>\n\
         Checksums-Sha1:\n da39a3ee5e6b4b0d3255bfef95601890afd80709 0 empty\n\
         Checksums-Sha256:\n e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 0 empty\n\
         Files:\n d41d8cd98f00b204e9800998ecf8427e 0 empty\n";

    fn dsc() -> Dsc {
        Dsc::parse(DSC).expect("Dsc fixture")
    }

    #[test]
    fn source() {
        let pass = vec![
            // Uses the substitution variable instead of libc.
            vec![(
                "debian/control",
                "Source: example\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Depends: ${shlibs:Depends}, ${misc:Depends}\n",
            )],
            // libc in Build-Depends is not read.
            vec![(
                "debian/control",
                "Source: example\n\
                 Build-Depends: libc6\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n",
            )],
            // The source package is glibc itself.
            vec![(
                "debian/control",
                "Source: glibc\n\
                 \n\
                 Package: libc6\n\
                 Architecture: any\n\
                 Depends: libc6, libc-bin\n",
            )],
            vec![(
                "debian/control",
                "Source: eglibc\n\
                 \n\
                 Package: libc0.3\n\
                 Architecture: any\n\
                 Pre-Depends: libc0.3\n",
            )],
            // A relation that names another package does not satisfy libc.
            vec![(
                "debian/control",
                "Source: example\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Depends: libc-dev\n",
            )],
        ];
        let fail = vec![
            vec![(
                "debian/control",
                "Source: example\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Depends: libc6 (>= 2.31)\n",
            )],
            vec![(
                "debian/control",
                "Source: control-file-hardcoded-libc\n\
                 Priority: optional\n\
                 Section: devel\n\
                 Maintainer: Debian Lintian Maintainers <lintian-maint@debian.org>\n\
                 Standards-Version: 4.7.4.1\n\
                 Build-Depends: debhelper-compat (= 14)\n\
                 Rules-Requires-Root: no\n\
                 \n\
                 Package: control-file-hardcoded-libc\n\
                 Architecture: all\n\
                 Pre-Depends: libc0.1\n\
                 Depends: ${shlibs:Depends}, ${misc:Depends}, libc6\n\
                 Recommends: libc6.1\n\
                 Suggests: libc0.3\n\
                 Description: Test hard-coded dependencies on libc\n extra\n",
            )],
            // Each installable is read, and the field name is matched without case.
            vec![(
                "debian/control",
                "Source: example\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 depends: libc6\n\
                 \n\
                 Package: example-udeb\n\
                 Package-Type: udeb\n\
                 Architecture: all\n\
                 Pre-Depends: libc6.1\n",
            )],
            // A comment line between fields keeps the line numbers of the file.
            vec![(
                "debian/control",
                "Source: example\n\
                 \n\
                 # comment\n\
                 Package: example\n\
                 Architecture: all\n\
                 # comment\n\
                 Depends: libc0.1\n",
            )],
        ];
        SourceTreeTester::new(PackageDependsOnHardcodedLibc, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![
            SourcePackageCase::from(vec![(
                "debian/control",
                "Source: example\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Depends: ${shlibs:Depends}\n",
            )])
            .dsc(dsc()),
        ];
        let fail = vec![
            SourcePackageCase::from(vec![(
                "debian/control",
                "Source: example\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Depends: libc6\n",
            )])
            .dsc(dsc()),
        ];
        SourcePackageTester::new(PackageDependsOnHardcodedLibc, pass, fail).test_and_snapshot();
    }
}
