use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct DependsOnMiscPreDepends;

declare_rule! {
    /// An installable's `Depends` in `debian/control` contains
    /// `${misc:Pre-Depends}`: the variable belongs in `Pre-Depends`.
    ///
    /// Each installable is read in file order. Extra is the `Depends` value
    /// (trimmed) followed by `(in section for <installable>)`. Location is
    /// `debian/control` with the line of the `Depends` field (no line when the
    /// raw file has no paragraph naming the installable). Only `Depends` is read, so a `${misc:Pre-Depends}` in
    /// `Pre-Depends` itself is not reported. The source paragraph is skipped.
    DependsOnMiscPreDepends,
    code = "LN1136",
    tag = "depends-on-misc-pre-depends",
    default_selected = true,
    experimental = false,
}

const VARIABLE: &str = "${misc:Pre-Depends}";

impl SourceRule for DependsOnMiscPreDepends {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };

        let mut reports = Vec::new();
        for installable in control.installables() {
            let Some(depends) = installable.fields().get("Depends") else {
                continue;
            };
            if !depends.contains(VARIABLE) {
                continue;
            }
            let line = installable.fields().position("Depends");
            reports.push((
                format!("{depends} (in section for {})", installable.name()),
                line,
            ));
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
            vec![(
                "debian/control",
                "Source: example\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Depends: ${misc:Depends}\n\
                 Pre-Depends: ${misc:Pre-Depends}\n",
            )],
            // The variable in the source paragraph's Depends is not an installable.
            vec![(
                "debian/control",
                "Source: example\n\
                 Depends: ${misc:Pre-Depends}\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n",
            )],
            // A Build-Depends is not read.
            vec![(
                "debian/control",
                "Source: example\n\
                 Build-Depends: ${misc:Pre-Depends}\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n",
            )],
        ];
        let fail = vec![
            vec![(
                "debian/control",
                "Source: depends-on-misc-pre-depends\n\
                 Priority: optional\n\
                 Section: devel\n\
                 Maintainer: Debian Lintian Maintainers <lintian-maint@debian.org>\n\
                 Standards-Version: 4.7.4.1\n\
                 Build-Depends: debhelper-compat (= 14)\n\
                 Rules-Requires-Root: no\n\
                 Homepage: https://lintian.debian.org/\n\
                 \n\
                 Package: depends-on-misc-pre-depends\n\
                 Architecture: all\n\
                 Depends: ${misc:Depends}, ${misc:Pre-Depends}\n\
                 Description: Check for packages Depending on ${misc:Pre-Depends}\n extra\n",
            )],
            // Each installable is read in turn; the variable is found in each.
            vec![(
                "debian/control",
                "Source: example\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Depends: foo, ${misc:Pre-Depends}\n\
                 \n\
                 Package: example-udeb\n\
                 Package-Type: udeb\n\
                 Architecture: all\n\
                 Depends: ${misc:Pre-Depends}\n",
            )],
        ];
        SourceTreeTester::new(DependsOnMiscPreDepends, pass, fail).test_and_snapshot();
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
                 Pre-Depends: ${misc:Pre-Depends}\n",
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
                 Depends: ${misc:Pre-Depends}\n",
            )])
            .dsc(dsc()),
        ];
        SourcePackageTester::new(DependsOnMiscPreDepends, pass, fail).test_and_snapshot();
    }
}
