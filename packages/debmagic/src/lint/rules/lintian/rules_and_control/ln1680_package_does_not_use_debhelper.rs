use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::{SourceContext, follow_symlinks};
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

pub struct PackageDoesNotUseDebhelper;

declare_rule! {
    /// The package does not appear to use debhelper. Reported at pedantic
    /// severity with an empty extra.
    ///
    /// A `Build-Driver` other than `debian-rules` in the Source paragraph
    /// is reported at that field's line in `debian/control`, as lintian does.
    /// Otherwise `debian/rules` (followed through symlinks inside the tree)
    /// must be readable, and a `debian/rules` with no debhelper `dh` command
    /// is reported with the `debian/rules` Location and no line. A missing or
    /// unreadable `debian/rules` is not reported.
    PackageDoesNotUseDebhelper,
    code = "LN1680",
    tag = "package-does-not-use-debhelper",
    default_selected = false,
    experimental = false,
}

impl SourceRule for PackageDoesNotUseDebhelper {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let (driver, driver_line) = match ctx.debian_control() {
            Some(control) => (
                control.source().get("Build-Driver").map(str::to_owned),
                control.source().position("Build-Driver"),
            ),
            None => return,
        };
        if let Some(driver) = driver
            && driver != "debian-rules"
        {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path: PathBuf::from("debian/control"),
                        line: driver_line,
                        column: None,
                    }),
            );
            return;
        }

        let readable = {
            let Some(files) = ctx.files() else {
                return;
            };
            files
                .get("debian/rules")
                .and_then(|entry| follow_symlinks(files, entry))
                .is_some_and(|target| files.read(target).is_ok())
        };
        if !readable {
            return;
        }
        if ctx.debhelper().is_some_and(|facts| facts.uses_debhelper()) {
            return;
        }
        ctx.diagnostic(
            Diagnostic::warn("")
                .with_severity(Severity::Pedantic)
                .with_location(Location {
                    path: PathBuf::from("debian/rules"),
                    line: None,
                    column: None,
                }),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn tree(driver: Option<&str>, rules: Option<&str>) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\n\
                 Maintainer: Example <ex@example.com>\n\
                 Standards-Version: 4.7.2\n\
                 {}\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n",
                driver
                    .map(|driver| format!("Build-Driver: {driver}\n"))
                    .unwrap_or_default()
            )),
        )];
        if let Some(rules) = rules {
            files.push(("debian/rules", TestFile::new(rules)));
        }
        files
    }

    const DH: &str = "#!/usr/bin/make -f\n%:\n\tdh $@\n";
    const PLAIN: &str = "#!/usr/bin/make -f\nbuild:\n\t./configure\n";

    #[test]
    fn test() {
        let pass = vec![
            tree(None, Some(DH)),
            tree(Some("debian-rules"), Some(DH)),
            // No debian/rules: nothing is read, nothing is reported.
            tree(None, None),
        ];
        let fail = vec![
            tree(None, Some(PLAIN)),
            tree(Some("debian-rules"), Some(PLAIN)),
            // A Build-Driver other than debian-rules is reported at its line.
            tree(Some("cmake"), Some(DH)),
        ];
        SourceTreeTester::new(PackageDoesNotUseDebhelper, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(None, Some(DH)))];
        let fail = vec![SourcePackageCase::from(tree(None, Some(PLAIN)))];
        SourcePackageTester::new(PackageDoesNotUseDebhelper, pass, fail).test_and_snapshot();
    }
}
