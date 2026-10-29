use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debhelper::DEPRECATED_COMPAT_LEVEL;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct PackageUsesDeprecatedDebhelperCompatVersion;

declare_rule! {
    /// The debhelper compat level is below lintian's deprecated threshold.
    ///
    /// Emitted only after `debian/rules` shows a dh command (`dh`, `dh_*`
    /// other than `dh_autoreconf`, or an include of `pkg-pecl.mk` /
    /// `blends-dev/rules`). The level is the declared compat level: the first
    /// line of `debian/compat`, else `Build-Depends: debhelper-compat (= N)`,
    /// else `X-DH-Compat`. Nothing declared, or a declaration `dh_assistant`
    /// would reject, is level 1. A `Build-Driver` other than `debian-rules`
    /// skips the Tag. Extra is the level. Applies to the Source tree and
    /// Source package.
    PackageUsesDeprecatedDebhelperCompatVersion,
    code = "LN0019",
    tag = "package-uses-deprecated-debhelper-compat-version",
    default_selected = true,
    experimental = false,
}

impl SourceRule for PackageUsesDeprecatedDebhelperCompatVersion {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = ctx.debhelper() else {
            return;
        };
        if !facts.uses_debhelper() {
            return;
        }
        let level = facts.level();
        if level < DEPRECATED_COMPAT_LEVEL {
            ctx.diagnostic(Diagnostic::warn(level.to_string()));
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

    fn package(control: &str, rules: &str, compat: Option<&str>) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![
            ("debian/control", TestFile::new(control)),
            ("debian/rules", TestFile::new(rules)),
        ];
        if let Some(compat) = compat {
            files.push(("debian/compat", TestFile::new(compat)));
        }
        files
    }

    #[test]
    fn test() {
        let dh = "#!/usr/bin/make -f\n%:\n\tdh $@\n";
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            package(CONTROL, "#!/usr/bin/make -f\nbuild:\n\ttrue\n", Some("1\n")),
            package(CONTROL, "#\tdh $@\n\tdh_autoreconf\n", None),
            package(CONTROL, dh, Some("10\n")),
            package(CONTROL, dh, Some("13\n")),
            package(
                "Source: example\n\
                 Build-Depends: debhelper-compat (= 13)\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n",
                dh,
                None,
            ),
            package(
                "Source: example\n\
                 X-DH-Compat: 10\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n",
                dh,
                None,
            ),
            package(
                "Source: example\n\
                 Build-Driver: debputy\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n",
                dh,
                None,
            ),
            // tar-ignore drops the file from the source package, not from the tree.
            vec![
                ("debian/control", TestFile::new(CONTROL)),
                ("debian/rules", TestFile::new(dh)),
                ("debian/compat", TestFile::new("13\n")),
                (
                    "debian/source/options",
                    TestFile::new("tar-ignore\ntar-ignore=*/compat\n"),
                ),
            ],
            package(CONTROL, " include /usr/share/dh-php/pkg-pecl.mk\n", None),
            package(CONTROL, "\tdh$@\n", None),
        ];
        let fail = vec![
            package(CONTROL, dh, Some("9\n")),
            package(CONTROL, dh, Some("8\n")),
            package(CONTROL, dh, Some("  9  \n")),
            package(CONTROL, dh, Some("\n")),
            package(CONTROL, dh, None),
            package(
                "Source: example\n\
                 Build-Depends: debhelper (>= 13~)\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n",
                dh,
                None,
            ),
            package(CONTROL, "\tdh_testroot # dummy\n", None),
            package(CONTROL, "include /usr/share/blends-dev/rules\n", None),
            package(CONTROL, "ifeq (1,1)\n\tdh $@\nendif\n", None),
            package(
                "Source: example\n\
                 Build-Depends: debhelper-compat (= 9)\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n",
                dh,
                None,
            ),
            package(
                "Source: example\n\
                 X-DH-Compat: 9\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n",
                dh,
                None,
            ),
            // Conflicting declarations abort; the level becomes 1.
            package(
                "Source: example\n\
                 Build-Depends: debhelper-compat (= 13)\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n",
                dh,
                Some("9\n"),
            ),
            package(CONTROL, dh, Some("14\n")),
            package(CONTROL, dh, Some("15\n")),
            package(CONTROL, dh, Some("0\n")),
            package(CONTROL, "\t-dh_clideps\n", None),
            package(CONTROL, "\tFOO=bar dh clean\n", None),
            vec![
                ("debian/rules", TestFile::new(dh)),
                ("debian/compat", TestFile::new("9\n")),
            ],
            vec![
                ("debian/control", TestFile::new(CONTROL)),
                ("debian/rules", TestFile::new(dh)),
                ("debian/compat", TestFile::symlink("level")),
                ("debian/level", TestFile::new("8\n")),
            ],
        ];
        SourceTreeTester::new(PackageUsesDeprecatedDebhelperCompatVersion, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let dh = "#!/usr/bin/make -f\n%:\n\tdh $@\n";
        let pass = vec![SourcePackageCase::from(package(CONTROL, dh, Some("13\n")))];
        let fail = vec![SourcePackageCase::from(package(CONTROL, dh, Some("9\n")))];
        SourcePackageTester::new(PackageUsesDeprecatedDebhelperCompatVersion, pass, fail)
            .test_and_snapshot();
    }
}
