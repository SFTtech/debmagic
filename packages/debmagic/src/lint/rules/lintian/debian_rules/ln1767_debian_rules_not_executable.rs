use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::text::follow_rules;

pub struct DebianRulesNotExecutable;

declare_rule! {
    /// `debian/rules` is not marked executable.
    ///
    /// Follows `debian/rules` through symbolic links (as
    /// `debian-rules-not-a-makefile` does) and reports the followed file when
    /// none of the three execute bits is set. Only a regular file or a
    /// hardlink is judged; a chain that ends at a directory, a special file,
    /// a dangling or absolute link, or a missing file reports nothing. Extra
    /// is empty. Location is the followed file with no line: `debian/rules`,
    /// or the link target when `debian/rules` is a link. Pedantic and not
    /// selected by default. Applies to the Source tree and Source package.
    DebianRulesNotExecutable,
    code = "LN1767",
    tag = "debian-rules-not-executable",
    default_selected = false,
    experimental = false,
}

impl SourceRule for DebianRulesNotExecutable {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some((path, entry)) = follow_rules(files) else {
            return;
        };
        if entry.is_executable_file() {
            return;
        }
        ctx.diagnostic(
            Diagnostic::warn("")
                .with_severity(Severity::Pedantic)
                .with_location(Location {
                    path,
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

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            vec![(
                "debian/rules",
                TestFile::new("#!/usr/bin/make -f\n").mode(0o755),
            )],
            // Any one execute bit is enough, as lintian's `is_executable`.
            vec![(
                "debian/rules",
                TestFile::new("#!/usr/bin/make -f\n").mode(0o100),
            )],
            // The link is followed to an executable file.
            vec![
                ("debian/rules", TestFile::symlink("real")),
                (
                    "debian/real",
                    TestFile::new("#!/usr/bin/make -f\n").mode(0o755),
                ),
            ],
            // Nothing to judge: a directory, a dangling link, an absolute link.
            vec![("debian/rules", TestFile::directory())],
            vec![("debian/rules", TestFile::symlink("missing"))],
            vec![("debian/rules", TestFile::symlink("/usr/bin/make"))],
            // A link to a directory ends the chain without a hint.
            vec![
                ("debian/rules", TestFile::symlink("dir")),
                ("debian/dir", TestFile::directory()),
            ],
        ];
        let fail = vec![
            vec![(
                "debian/rules",
                TestFile::new("#!/usr/bin/make -f\n").mode(0o644),
            )],
            // A non-executable link target is reported at the target path.
            vec![
                ("debian/rules", TestFile::symlink("real")),
                (
                    "debian/real",
                    TestFile::new("#!/usr/bin/make -f\n").mode(0o644),
                ),
            ],
        ];
        SourceTreeTester::new(DebianRulesNotExecutable, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(vec![(
            "debian/rules",
            TestFile::new("#!/usr/bin/make -f\n").mode(0o755),
        )])];
        let fail = vec![SourcePackageCase::from(vec![(
            "debian/rules",
            TestFile::new("#!/usr/bin/make -f\n").mode(0o644),
        )])];
        SourcePackageTester::new(DebianRulesNotExecutable, pass, fail).test_and_snapshot();
    }
}
