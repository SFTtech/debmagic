use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::InstallablePackageType;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct DebhelperButNoMiscDepends;

declare_rule! {
    /// An installable built with debhelper does not depend on `${misc:Depends}`.
    ///
    /// Emitted only after `debian/rules` shows a dh command and `Build-Driver`
    /// is `debian-rules` or absent. Each `deb` installable is checked;
    /// `udeb` paragraphs are skipped. `${misc:Depends}` may sit in
    /// `Pre-Depends`, `Depends`, `Recommends`, or `Suggests`. Compat 14 and
    /// above, and a build dependency on `dh-sequence-debputy`,
    /// `dh-sequence-zz-debputy`, or `dh-sequence-zz-debputy-rrr`, suppress the
    /// Tag. Extra is the installable name. Applies to the Source tree and
    /// Source package.
    DebhelperButNoMiscDepends,
    code = "LN0449",
    tag = "debhelper-but-no-misc-depends",
    default_selected = true,
    experimental = false,
}

/// First compat level at which debhelper adds `${misc:Depends}` itself.
const MISC_DEPENDS_AUTO_APPLY: u32 = 14;

const MISC_DEPENDS: &str = "${misc:Depends}";

const BUILD_DEPENDS: &[&str] = &["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"];

const INSTALLATION: &[&str] = &["Pre-Depends", "Depends", "Recommends", "Suggests"];

const DEBPUTY: &[&str] = &[
    "dh-sequence-debputy",
    "dh-sequence-zz-debputy",
    "dh-sequence-zz-debputy-rrr",
];

impl SourceRule for DebhelperButNoMiscDepends {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = ctx.debhelper() else {
            return;
        };
        if !facts.uses_debhelper() || facts.level() >= MISC_DEPENDS_AUTO_APPLY {
            return;
        }
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let build = control.source().relations_and(BUILD_DEPENDS);
        if DEBPUTY.iter().any(|package| build.satisfies(package)) {
            return;
        }
        let names: Vec<String> = control
            .installables()
            .iter()
            .filter(|installable| installable.package_type() == InstallablePackageType::Deb)
            .filter(|installable| {
                !installable
                    .fields()
                    .relations_and(INSTALLATION)
                    .satisfies(MISC_DEPENDS)
            })
            .map(|installable| installable.name().to_string())
            .collect();
        for name in names {
            ctx.diagnostic(Diagnostic::warn(name));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const DH: &str = "#!/usr/bin/make -f\n%:\n\tdh $@\n";

    fn tree(control: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(control)),
            ("debian/rules", TestFile::new(DH)),
            ("debian/compat", TestFile::new("13\n")),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree(
                "\
Source: example\n\
\n\
Package: example\n\
Architecture: all\n\
Depends: ${misc:Depends}\n\
Description: example\n extra\n",
            ),
            tree(
                "\
Source: example\n\
\n\
Package: example\n\
Architecture: all\n\
Recommends: other, ${misc:Depends}\n\
Description: example\n extra\n",
            ),
            tree(
                "\
Source: example\n\
Build-Depends: dh-sequence-debputy\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n",
            ),
            vec![
                (
                    "debian/control",
                    TestFile::new(
                        "\
Source: example\n\
Build-Depends: debhelper-compat (= 14)\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n",
                    ),
                ),
                ("debian/rules", TestFile::new(DH)),
            ],
            tree(
                "\
Source: example\n\
\n\
Package: example-udeb\n\
Package-Type: udeb\n\
Architecture: any\n\
Description: udeb\n extra\n",
            ),
            vec![
                (
                    "debian/control",
                    TestFile::new(
                        "\
Source: example\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n",
                    ),
                ),
                (
                    "debian/rules",
                    TestFile::new("#!/usr/bin/make -f\nbuild:\n\ttrue\n"),
                ),
            ],
        ];
        let fail = vec![
            tree(
                "\
Source: example\n\
\n\
Package: example\n\
Architecture: all\n\
Depends: ${shlibs:Depends}\n\
Description: example\n extra\n",
            ),
            tree(
                "\
Source: example\n\
\n\
Package: first\n\
Architecture: all\n\
Depends: something | ${misc:Depends}\n\
Description: first\n extra\n\
\n\
Package: second\n\
Architecture: all\n\
Description: second\n extra\n",
            ),
            tree(
                "\
Source: example\n\
\n\
Package: example\n\
Architecture: all\n\
Pre-Depends: ${misc:Depends}\n\
Description: kept\n extra\n\
\n\
Package: example-udeb\n\
Package-Type: udeb\n\
Architecture: any\n\
Description: udeb\n extra\n\
\n\
Package: other\n\
Architecture: all\n\
Description: other\n extra\n",
            ),
        ];
        SourceTreeTester::new(DebhelperButNoMiscDepends, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            "\
Source: example\n\
\n\
Package: example\n\
Architecture: all\n\
Depends: ${misc:Depends}\n\
Description: example\n extra\n",
        ))];
        let fail = vec![SourcePackageCase::from(tree(
            "\
Source: example\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n",
        ))];
        SourcePackageTester::new(DebhelperButNoMiscDepends, pass, fail).test_and_snapshot();
    }
}
