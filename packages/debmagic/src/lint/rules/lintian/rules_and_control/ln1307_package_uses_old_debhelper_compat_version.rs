use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debhelper::{deprecated_compat_level, recommended_compat_level};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

pub struct PackageUsesOldDebhelperCompatVersion;

declare_rule! {
    /// The debhelper compat level is at or above lintian's deprecated threshold
    /// but below its recommended one, so the package does not declare the
    /// recommended `debhelper-compat` level yet.
    ///
    /// Emitted under the same conditions as `package-uses-deprecated-debhelper-compat-version`
    /// (see that Rule): `debian/rules` shows a dh command and the declared
    /// level comes from `debian/compat`, `Build-Depends: debhelper-compat (= N)`
    /// or `X-DH-Compat`; nothing declared is level 1. A `Build-Driver` other
    /// than `debian-rules` skips the Tag. Extra is the level. Pedantic and not
    /// selected by default, as in lintian. Applies to the Source tree and
    /// Source package.
    PackageUsesOldDebhelperCompatVersion,
    code = "LN1307",
    tag = "package-uses-old-debhelper-compat-version",
    default_selected = false,
    experimental = false,
}

impl SourceRule for PackageUsesOldDebhelperCompatVersion {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = ctx.debhelper() else {
            return;
        };
        if !facts.uses_debhelper() {
            return;
        }
        let level = facts.level();
        if (deprecated_compat_level()..recommended_compat_level()).contains(&level) {
            ctx.diagnostic(Diagnostic::warn(level.to_string()).with_severity(Severity::Pedantic));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n";

    fn package(compat: Option<&str>) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![
            ("debian/control", TestFile::new(CONTROL)),
            (
                "debian/rules",
                TestFile::new("#!/usr/bin/make -f\n%:\n\tdh $@\n"),
            ),
        ];
        if let Some(compat) = compat {
            files.push(("debian/compat", TestFile::new(compat)));
        }
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            // Deprecated levels are LN0019's; the recommended level is not old.
            package(Some("9\n")),
            package(Some("14\n")),
            package(None),
        ];
        let fail = vec![package(Some("10\n")), package(Some("13\n"))];
        SourceTreeTester::new(PackageUsesOldDebhelperCompatVersion, pass, fail).test_and_snapshot();
    }
}
