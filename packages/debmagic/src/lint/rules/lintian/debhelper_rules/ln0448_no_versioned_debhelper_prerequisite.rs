use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debhelper::CompatSource;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct NoVersionedDebhelperPrerequisite;

declare_rule! {
    /// Debhelper is used without a versioned `debhelper` build dependency
    /// high enough for the compat level.
    ///
    /// Emitted only after `debian/rules` shows a dh command and `Build-Driver`
    /// is `debian-rules` or absent. The level is the declared compat level
    /// (`debian/compat`, else `debhelper-compat (= N)`, else `X-DH-Compat`;
    /// nothing declared is 1). `Build-Depends`, `Build-Depends-Indep`, and
    /// `Build-Depends-Arch` together must imply `debhelper (>= N~)`, except
    /// that `X-DH-Compat` is compared against `13.15~` while the extra stays
    /// the declared level. `debhelper-compat (= N)` suppresses the Tag.
    /// Extra is the level. Applies to the Source tree and Source package.
    NoVersionedDebhelperPrerequisite,
    code = "LN0448",
    tag = "no-versioned-debhelper-prerequisite",
    default_selected = true,
    experimental = false,
}

/// Level `X-DH-Compat` is measured against, whatever number it declares.
const X_DH_COMPAT_PREREQUISITE: &str = "13.15";

const BUILD_DEPENDS: &[&str] = &["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"];

impl SourceRule for NoVersionedDebhelperPrerequisite {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = ctx.debhelper() else {
            return;
        };
        if !facts.uses_debhelper()
            || matches!(facts.compat_source(), Some(CompatSource::BuildDepends(_)))
        {
            return;
        }
        let level = facts.level();
        let required = if matches!(facts.compat_source(), Some(CompatSource::XDhCompat(_))) {
            X_DH_COMPAT_PREREQUISITE.to_string()
        } else {
            level.to_string()
        };
        let Some(control) = ctx.debian_control() else {
            return;
        };
        if control
            .source()
            .relations_and(BUILD_DEPENDS)
            .satisfies(&format!("debhelper (>= {required}~)"))
        {
            return;
        }
        ctx.diagnostic(Diagnostic::warn(level.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const DH: &str = "#!/usr/bin/make -f\n%:\n\tdh $@\n";

    fn package(control: &str, compat: Option<&str>) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![
            ("debian/control", TestFile::new(control)),
            ("debian/rules", TestFile::new(DH)),
        ];
        if let Some(compat) = compat {
            files.push(("debian/compat", TestFile::new(compat)));
        }
        files
    }

    fn control(build_depends: &str) -> String {
        format!(
            "\
Source: example\n\
Build-Depends: {build_depends}\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n"
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            package(&control("debhelper (>= 13~)"), Some("13\n")),
            package(&control("debhelper (>= 9)"), Some("9\n")),
            package(&control("debhelper (>= 14)"), Some("13\n")),
            package(&control("debhelper-compat (= 13)"), None),
            package(&control("debhelper (>= 13.15~)"), None),
            vec![
                (
                    "debian/control",
                    TestFile::new(control("debhelper (>= 1~)")),
                ),
                (
                    "debian/rules",
                    TestFile::new("#!/usr/bin/make -f\nbuild:\n\ttrue\n"),
                ),
            ],
            package(
                "\
Source: example\n\
Build-Driver: debputy\n\
Build-Depends: debhelper\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n",
                Some("9\n"),
            ),
        ];
        let fail = vec![
            package(&control("debhelper"), Some("9\n")),
            package(&control("debhelper (>= 8)"), Some("9\n")),
            package(&control("debhelper | dh-exec"), Some("13\n")),
            package(
                "\
Source: example\n\
X-DH-Compat: 9\n\
Build-Depends: debhelper (>= 13)\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n",
                None,
            ),
            package(&control("debhelper (>= 13) [amd64]"), Some("13\n")),
        ];
        SourceTreeTester::new(NoVersionedDebhelperPrerequisite, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(
            &control("debhelper (>= 13~)"),
            Some("13\n"),
        ))];
        let fail = vec![SourcePackageCase::from(package(
            &control("debhelper"),
            Some("9\n"),
        ))];
        SourcePackageTester::new(NoVersionedDebhelperPrerequisite, pass, fail).test_and_snapshot();
    }
}
