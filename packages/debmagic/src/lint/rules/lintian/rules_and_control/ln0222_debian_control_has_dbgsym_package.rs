use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct DebianControlHasDbgsymPackage;

declare_rule! {
    /// `debian/control` still declares a `-dbgsym` binary package.
    ///
    /// Each installable whose name ends in `-dbgsym` is reported. Extra is
    /// `(in section for <package>)` then `Package`. Location is `debian/control`
    /// at the `Package` field. Applies to the Source tree and Source package.
    /// A missing or unparseable `debian/control` has no installables.
    DebianControlHasDbgsymPackage,
    code = "LN0222",
    tag = "debian-control-has-dbgsym-package",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianControlHasDbgsymPackage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let names: Vec<(String, Option<u32>)> = ctx
            .debian_control()
            .map(|control| {
                control
                    .installables()
                    .iter()
                    .filter(|installable| installable.name().ends_with("-dbgsym"))
                    .map(|installable| {
                        let line = installable.fields().position("Package");
                        (installable.name().to_string(), line)
                    })
                    .collect()
            })
            .unwrap_or_default();
        if names.is_empty() {
            return;
        }
        for (name, line) in names {
            ctx.diagnostic(
                Diagnostic::error(format!("(in section for {name}) Package")).with_location(
                    Location {
                        path: PathBuf::from("debian/control"),
                        line,
                        column: None,
                    },
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(text: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/control", TestFile::new(text))]
    }

    const PLAIN: &str = "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n";

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no control"))],
            control(PLAIN),
            control(
                "\
Source: example\n\
\n\
Package: example-dbg\n\
Architecture: all\n\
Description: manual dbg\n package\n",
            ),
            control(
                "\
Source: example\n\
\n\
Package: dbgsym\n\
Architecture: all\n\
Description: not a suffix\n package\n",
            ),
            control(
                "\
Source: example\n\
\n\
Package: example-dbgsym-utils\n\
Architecture: all\n\
Description: not the suffix\n package\n",
            ),
            control(
                "\
Source: example-dbgsym\n\
\n\
Package: example\n\
Architecture: all\n\
Description: source name only\n package\n",
            ),
            control(
                "\
Source: example\n\
\n\
Package: example-DBGSYM\n\
Architecture: all\n\
Description: case sensitive\n package\n",
            ),
        ];
        let fail = vec![
            control(
                "\
Source: example\n\
\n\
Package: example-dbgsym\n\
Architecture: all\n\
Description: manual dbgsym\n package\n",
            ),
            control(
                "\
Source: foo++\n\
\n\
Package: foo++\n\
Architecture: all\n\
Description: helper\n package\n\
\n\
Package: foo++-dbgsym\n\
Architecture: all\n\
Description: symbols\n package\n",
            ),
            // The pointer is the Package field, not the first line of the stanza.
            control(
                "\
Source: example\n\
\n\
Section: debug\n\
Package: example-dbgsym\n\
Architecture: all\n\
Description: symbols\n package\n",
            ),
            control(
                "\
Source: example\n\
\n\
Package: example-dbgsym\n\
Architecture: all\n\
Description: one\n package\n\
\n\
Package: other-dbgsym\n\
Architecture: all\n\
Description: two\n package\n",
            ),
        ];
        SourceTreeTester::new(DebianControlHasDbgsymPackage, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(control(PLAIN))];
        let fail = vec![SourcePackageCase::from(control(
            "\
Source: example\n\
\n\
Package: example-dbgsym\n\
Architecture: all\n\
Description: symbols\n package\n",
        ))];
        SourcePackageTester::new(DebianControlHasDbgsymPackage, pass, fail).test_and_snapshot();
    }
}
