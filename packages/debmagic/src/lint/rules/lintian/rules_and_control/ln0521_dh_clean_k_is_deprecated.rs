use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dh_scan::gated_scan;

pub struct DhCleanKIsDeprecated;

declare_rule! {
    /// `debian/rules` calls `dh_clean -k`.
    ///
    /// Scans `debian/rules` the way lintian's `debhelper` Check does: `\`
    /// continuations are joined first, then comment lines are dropped. An
    /// indented `dh_clean` command whose line carries a `-k` argument
    /// (whitespace before it, whitespace or the end of the line after it)
    /// reports once for the whole file. Only reached after a dh command has
    /// been seen; a `Build-Driver` other than `debian-rules` is not scanned.
    /// Extra is empty. Location is `debian/rules` with no line. Applies to the
    /// Source tree and Source package.
    DhCleanKIsDeprecated,
    code = "LN0521",
    tag = "dh-clean-k-is-deprecated",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DhCleanKIsDeprecated {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some((_, scan)) = gated_scan(ctx) else {
            return;
        };
        if scan.clean_k {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: PathBuf::from("debian/rules"),
                line: None,
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

    fn package(rules: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/rules", TestFile::new(rules)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            package("%:\n\tdh $@\n\tdh_clean\n"),
            package("%:\n\tdh $@\n\tdh_clean -X.orig\n"),
            package("%:\n\tdh $@\n\t# dh_clean -k\n"),
            package("%:\n\tdh $@\n\tdh_clean -kx\n"),
            package("%:\n\tdh $@\n\tdh_prep -k\n"),
            // Not indented, so not a command.
            package("%:\n\tdh $@\ndh_clean -k\n"),
            // No dh command was seen.
            package("clean:\n\ttrue\n"),
        ];
        let fail = vec![
            package("%:\n\tdh $@\n\tdh_clean -k\n"),
            package("%:\n\tdh $@\n\tdh_clean -a -k\n"),
            package("%:\n\tdh $@\n\t@LC_ALL=C dh_clean -k -X.orig\n"),
            package("%:\n\tdh $@\n\tdh_clean \\\n\t  -k\n"),
            package("clean:\n\tdh_clean -k\n\tdh_clean -k\n"),
        ];
        SourceTreeTester::new(DhCleanKIsDeprecated, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package("%:\n\tdh_clean\n"))];
        let fail = vec![SourcePackageCase::from(package("%:\n\tdh_clean -k\n"))];
        SourcePackageTester::new(DhCleanKIsDeprecated, pass, fail).test_and_snapshot();
    }
}
