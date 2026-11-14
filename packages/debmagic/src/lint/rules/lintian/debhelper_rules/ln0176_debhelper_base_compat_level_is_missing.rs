use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debhelper::CompatSource;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct DebhelperBaseCompatLevelIsMissing;

declare_rule! {
    /// Debhelper is used, but no compat level is declared.
    ///
    /// Emitted only after `debian/rules` shows a dh command (`dh`, `dh_*`
    /// other than `dh_autoreconf`, or an include of `pkg-pecl.mk` /
    /// `blends-dev/rules`) and `Build-Driver` is `debian-rules` or absent. A
    /// `debian/compat` that can be opened suppresses the Tag, including an
    /// empty or non-numeric file. So does a level taken from
    /// `Build-Depends: debhelper-compat (= N)` or `X-DH-Compat`. A source tree
    /// with a complete `debian/control` and no `debian/rules` does not emit.
    /// Extra is empty and there is no Location. Applies to the Source tree
    /// and Source package.
    DebhelperBaseCompatLevelIsMissing,
    code = "LN0176",
    tag = "debhelper-base-compat-level-is-missing",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebhelperBaseCompatLevelIsMissing {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = ctx.debhelper() else {
            return;
        };
        if !facts.uses_debhelper() || facts.compat_file_open() {
            return;
        }
        if matches!(
            facts.compat_source(),
            Some(CompatSource::BuildDepends(_) | CompatSource::XDhCompat(_))
        ) {
            return;
        }
        ctx.diagnostic(Diagnostic::warn(""));
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

    const DH: &str = "#!/usr/bin/make -f\n%:\n\tdh $@\n";

    fn package(
        control: &str,
        rules: Option<&str>,
        compat: Option<TestFile>,
    ) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![("debian/control", TestFile::new(control))];
        if let Some(rules) = rules {
            files.push(("debian/rules", TestFile::new(rules)));
        }
        if let Some(compat) = compat {
            files.push(("debian/compat", compat));
        }
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            // The select_ln fixture: complete control, no debian/rules.
            package(CONTROL, None, None),
            package(CONTROL, Some("#!/usr/bin/make -f\nbuild:\n\ttrue\n"), None),
            package(CONTROL, Some("#\tdh $@\n\tdh_autoreconf\n"), None),
            package(CONTROL, Some(DH), Some(TestFile::new("13\n"))),
            package(CONTROL, Some(DH), Some(TestFile::new("\n"))),
            package(CONTROL, Some(DH), Some(TestFile::new("nope\n"))),
            package(CONTROL, Some(DH), Some(TestFile::new("0\n"))),
            package(
                "Source: example\n\
                 Build-Depends: debhelper-compat (= 13)\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n",
                Some(DH),
                None,
            ),
            package(
                "Source: example\n\
                 X-DH-Compat: 13\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n",
                Some(DH),
                None,
            ),
            // The file is open, so a conflicting declaration still suppresses.
            package(
                "Source: example\n\
                 Build-Depends: debhelper-compat (= 13)\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n",
                Some(DH),
                Some(TestFile::new("9\n")),
            ),
            package(
                "Source: example\n\
                 Build-Driver: debputy\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n",
                Some(DH),
                None,
            ),
            vec![
                ("debian/control", TestFile::new(CONTROL)),
                ("debian/rules", TestFile::new(DH)),
                ("debian/compat", TestFile::symlink("level")),
                ("debian/level", TestFile::new("13\n")),
            ],
        ];
        let fail = vec![
            package(CONTROL, Some(DH), None),
            package(
                "Source: example\n\
                 Build-Depends: debhelper (>= 13~)\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n",
                Some(DH),
                None,
            ),
            package(CONTROL, Some("\tdh_testroot\n"), None),
            package(
                CONTROL,
                Some("include /usr/share/dh-php/pkg-pecl.mk\n"),
                None,
            ),
            package(CONTROL, Some("include /usr/share/blends-dev/rules\n"), None),
            package(
                "Source: example\n\
                 X-DH-Compat: nope\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n",
                Some(DH),
                None,
            ),
            package(CONTROL, Some(DH), Some(TestFile::directory())),
            package(CONTROL, Some(DH), Some(TestFile::symlink("missing"))),
            // debhelper-compat in Build-Depends-Arch does not declare the level.
            package(
                "Source: example\n\
                 Build-Depends-Arch: debhelper-compat (= 13)\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n",
                Some(DH),
                None,
            ),
        ];
        SourceTreeTester::new(DebhelperBaseCompatLevelIsMissing, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            SourcePackageCase::from(package(CONTROL, None, None)),
            SourcePackageCase::from(package(CONTROL, Some(DH), Some(TestFile::new("13\n")))),
        ];
        let fail = vec![SourcePackageCase::from(package(CONTROL, Some(DH), None))];
        SourcePackageTester::new(DebhelperBaseCompatLevelIsMissing, pass, fail).test_and_snapshot();
    }
}
