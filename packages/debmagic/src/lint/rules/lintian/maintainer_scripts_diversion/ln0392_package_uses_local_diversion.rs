use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct PackageUsesLocalDiversion;

declare_rule! {
    /// A maintainer script calls `dpkg-divert --local`.
    ///
    /// Any interpreter; continuations join on every interpreter. `--help`,
    /// `--list`, `--truename`, and `--version` are not diversions. Extra is
    /// empty. The Location is the last physical line of the call.
    PackageUsesLocalDiversion,
    code = "LN0392",
    tag = "package-uses-local-diversion",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageUsesLocalDiversion {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(diversions) = ctx.diversions() else {
            return;
        };
        for diversion in diversions.declared().iter().filter(|d| d.is_local()) {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path: diversion.script().to_path_buf(),
                line: Some(diversion.line()),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn control(name: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(content))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            control(
                "preinst",
                "#!/bin/sh\ndpkg-divert --rename --package foo /bin/bar\n",
            ),
            control("preinst", "#!/bin/sh\ndpkg-divert --help --local\n"),
            control("preinst", "#!/bin/sh\n# dpkg-divert --local /bin/bar\n"),
            control("preinst", "dpkg-divert --local /bin/bar\n"),
            control("preinst", "#!/bin/sh\necho dpkg-divert --local /bin/bar\n"),
        ];
        let fail = vec![
            control(
                "preinst",
                "#!/bin/sh\ndpkg-divert --local --rename /bin/bar\n",
            ),
            control(
                "postrm",
                "#!/bin/sh\n/usr/sbin/dpkg-divert --local /bin/bar\n",
            ),
            // Continuations join on every interpreter.
            control(
                "postinst",
                "#!/usr/bin/perl\n`dpkg-divert --add \\\n  --local /bin/bar`;\n",
            ),
        ];
        BinaryPackageTester::new(PackageUsesLocalDiversion, pass, fail).test_and_snapshot();
    }
}
