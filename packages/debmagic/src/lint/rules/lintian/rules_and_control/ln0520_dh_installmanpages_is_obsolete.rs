use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dh_scan::gated_scan;

pub struct DhInstallmanpagesIsObsolete;

declare_rule! {
    /// `debian/rules` calls the obsolete `dh_installmanpages`.
    ///
    /// Scans `debian/rules` the way lintian's `debhelper` Check does: `\`
    /// continuations are joined first, then comment lines are dropped, and the
    /// counter lags after a continuation. An indented line whose command
    /// (after `@`, `+`, `-`, and `VAR=value` prefixes) is exactly
    /// `dh_installmanpages` reports once per line. Only reached after a dh
    /// command has been seen; a `Build-Driver` other than `debian-rules` is
    /// not scanned. Extra is empty. Location is `debian/rules` and the line.
    /// Applies to the Source tree and Source package.
    DhInstallmanpagesIsObsolete,
    code = "LN0520",
    tag = "dh_installmanpages-is-obsolete",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DhInstallmanpagesIsObsolete {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some((_, scan)) = gated_scan(ctx) else {
            return;
        };
        for position in scan.installmanpages {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: PathBuf::from("debian/rules"),
                line: Some(position),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n";

    fn package(control: &str, rules: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(control)),
            ("debian/rules", TestFile::new(rules)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            package(CONTROL, "%:\n\tdh $@\n"),
            package(CONTROL, "%:\n\tdh $@\n\t# dh_installmanpages\n"),
            // Not indented, so not a command.
            package(CONTROL, "%:\n\tdh $@\ndh_installmanpages\n"),
            // A different command.
            package(CONTROL, "%:\n\tdh $@\n\tdh_installmanpages2\n"),
            // No dh command was seen: a comment does not count.
            package(CONTROL, "build:\n\ttrue\n"),
            package(
                "\
Source: example\n\
Build-Driver: debputy\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n",
                "%:\n\tdh_installmanpages\n",
            ),
        ];
        let fail = vec![
            package(CONTROL, "%:\n\tdh $@\n\ndh_x:\n\tdh_installmanpages\n"),
            package(
                CONTROL,
                "build:\n\t-dh_installmanpages\n\t@LC_ALL=C dh_installmanpages -a\n\tdh_installmanpages \\\n\t  -a\nclean:\n\tdh_installmanpages\n",
            ),
        ];
        SourceTreeTester::new(DhInstallmanpagesIsObsolete, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(CONTROL, "%:\n\tdh $@\n"))];
        let fail = vec![SourcePackageCase::from(package(
            CONTROL,
            "%:\n\tdh_installmanpages\n",
        ))];
        SourcePackageTester::new(DhInstallmanpagesIsObsolete, pass, fail).test_and_snapshot();
    }
}
