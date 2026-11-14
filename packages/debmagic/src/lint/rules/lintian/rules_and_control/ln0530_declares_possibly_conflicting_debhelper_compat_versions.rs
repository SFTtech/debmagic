use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debhelper::CompatSource;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dh_scan::gated_scan;

pub struct DeclaresPossiblyConflictingDebhelperCompatVersions;

declare_rule! {
    /// `debian/rules` sets `DH_COMPAT` while a compat level is also declared.
    ///
    /// Scans `debian/rules` the way lintian's `debhelper` Check does: `\`
    /// continuations are joined first, then comment lines are dropped. An
    /// `export DH_COMPAT := N`, `DH_COMPAT := N` (later exported, or exported
    /// before), or `export DH_COMPAT` after a `DH_COMPAT` assignment gives a
    /// value; a `0` or empty value is ignored when it would replace another.
    /// Reported when a level is declared in `debian/compat`,
    /// `Build-Depends: debhelper-compat (= N)`, or `X-DH-Compat`; a package
    /// that declares nothing, or whose declaration `dh_assistant` rejects,
    /// does not report. Only reached after a dh command has been seen; a
    /// `Build-Driver` other than `debian-rules` is not scanned. Extra is the
    /// declared level, `from` and where it was declared, and `vs DH_COMPAT`
    /// and the value. Location is `debian/rules` with no line. Applies to the
    /// Source tree and Source package.
    DeclaresPossiblyConflictingDebhelperCompatVersions,
    code = "LN0530",
    tag = "declares-possibly-conflicting-debhelper-compat-versions",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DeclaresPossiblyConflictingDebhelperCompatVersions {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some((facts, scan)) = gated_scan(ctx) else {
            return;
        };
        if scan.dh_compat_rules.is_empty() {
            return;
        }
        let (Some(level), Some(source)) = (facts.declared_level(), facts.compat_source()) else {
            return;
        };
        let source = match source {
            CompatSource::DebianCompat => "debian/compat".to_string(),
            CompatSource::BuildDepends(level) => {
                format!("Build-Depends: debhelper-compat (= {level})")
            }
            CompatSource::XDhCompat(level) => format!("X-DH-Compat: {level}"),
        };
        ctx.diagnostic(
            Diagnostic::error(format!(
                "{level} from {source} vs DH_COMPAT {}",
                scan.dh_compat_rules
            ))
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

    fn control(build_depends: &str, extra: &str) -> String {
        format!(
            "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
{build_depends}{extra}\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n"
        )
    }

    fn package(
        control: String,
        rules: &str,
        compat: Option<&str>,
    ) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![
            ("debian/control", TestFile::new(control)),
            (
                "debian/rules",
                TestFile::new(format!("%:\n\tdh $@\n{rules}")),
            ),
        ];
        if let Some(compat) = compat {
            files.push(("debian/compat", TestFile::new(compat)));
        }
        files
    }

    #[test]
    fn test() {
        let plain = || control("", "");
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            // The assignment after the export is never exported.
            package(plain(), "export DH_COMPAT\nDH_COMPAT = 5\n", Some("9\n")),
            package(plain(), "DH_COMPAT ?= 7\nexport DH_COMPAT\n", Some("9\n")),
            // Nothing is declared, so there is nothing to conflict with.
            package(plain(), "export DH_COMPAT := 8\n", None),
            package(plain(), "", Some("13\n")),
            // A bare `export DH_COMPAT` before any assignment has no value.
            package(plain(), "export DH_COMPAT\n", Some("13\n")),
            package(plain(), "# export DH_COMPAT := 8\n", Some("13\n")),
            // dh_assistant rejects two declarations.
            package(
                control("Build-Depends: debhelper-compat (= 13)\n", ""),
                "export DH_COMPAT := 8\n",
                Some("13\n"),
            ),
        ];
        let fail = vec![
            package(plain(), "export DH_COMPAT := 8\n", Some("13\n")),
            package(plain(), "export DH_COMPAT=8\n", Some("12\n")),
            package(plain(), "DH_COMPAT := 7\nexport DH_COMPAT\n", Some("9\n")),
            package(plain(), "DH_COMPAT = 5\nexport DH_COMPAT\n", Some("9\n")),
            package(
                control("Build-Depends: debhelper-compat (= 13)\n", ""),
                "export DH_COMPAT := 8\n",
                None,
            ),
            package(
                control("", "X-DH-Compat: 14\n"),
                "export DH_COMPAT := 8\n",
                None,
            ),
            package(plain(), "export DH_COMPAT := 8\n", Some("0\n")),
        ];
        SourceTreeTester::new(
            DeclaresPossiblyConflictingDebhelperCompatVersions,
            pass,
            fail,
        )
        .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(
            control("", ""),
            "export DH_COMPAT := 8\n",
            None,
        ))];
        let fail = vec![SourcePackageCase::from(package(
            control("", ""),
            "export DH_COMPAT := 8\n",
            Some("13\n"),
        ))];
        SourcePackageTester::new(
            DeclaresPossiblyConflictingDebhelperCompatVersions,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
