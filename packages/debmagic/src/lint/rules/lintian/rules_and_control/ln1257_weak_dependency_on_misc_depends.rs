use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::InstallablePackageType;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct WeakDependencyOnMiscDepends;

declare_rule! {
    /// A `deb` installable depends on `${misc:Depends}` only weakly.
    ///
    /// Emitted only after `debian/rules` shows a dh command and `Build-Driver`
    /// is `debian-rules` or absent. The installable's `Pre-Depends`,
    /// `Depends`, `Recommends`, or `Suggests` mention `${misc:Depends}`, but
    /// its `Pre-Depends` and `Depends` do not. `udeb` paragraphs are skipped.
    /// Compat 14 and above, and a build dependency on `dh-sequence-debputy`,
    /// `dh-sequence-zz-debputy`, or `dh-sequence-zz-debputy-rrr`, suppress the
    /// Tag. Extra is the installable name. Applies to the Source tree and
    /// Source package.
    WeakDependencyOnMiscDepends,
    code = "LN1257",
    tag = "weak-dependency-on-misc-depends",
    default_selected = true,
    experimental = false,
}

/// First compat level at which debhelper adds `${misc:Depends}` itself.
const MISC_DEPENDS_AUTO_APPLY: u32 = 14;
const MISC_DEPENDS: &str = "${misc:Depends}";
const BUILD_DEPENDS: &[&str] = &["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"];
const INSTALLATION: &[&str] = &["Pre-Depends", "Depends", "Recommends", "Suggests"];
const STRONG: &[&str] = &["Pre-Depends", "Depends"];
const DEBPUTY: &[&str] = &[
    "dh-sequence-debputy",
    "dh-sequence-zz-debputy",
    "dh-sequence-zz-debputy-rrr",
];

impl SourceRule for WeakDependencyOnMiscDepends {
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
                let fields = installable.fields();
                fields.relations_and(INSTALLATION).satisfies(MISC_DEPENDS)
                    && !fields.relations_and(STRONG).satisfies(MISC_DEPENDS)
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

    fn control(packages: &str, build_depends: &str) -> String {
        format!(
            "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
Build-Depends: {build_depends}\n\
\n\
{packages}"
        )
    }

    fn tree(control: &str, compat: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(control)),
            ("debian/rules", TestFile::new(DH)),
            ("debian/compat", TestFile::new(compat)),
        ]
    }

    fn binary(name: &str, relations: &str) -> String {
        format!("Package: {name}\nArchitecture: all\n{relations}Description: example\n extra\n\n")
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            // Strong dependency on the substvar.
            tree(
                &control(
                    &binary("foo", "Depends: ${misc:Depends}\n"),
                    "debhelper (>= 13)",
                ),
                "13\n",
            ),
            // Pre-Depends is strong too.
            tree(
                &control(
                    &binary("foo", "Pre-Depends: ${misc:Depends}\n"),
                    "debhelper (>= 13)",
                ),
                "13\n",
            ),
            // Not mentioned at all: debhelper-but-no-misc-depends, not this Tag.
            tree(
                &control(&binary("foo", "Depends: bar\n"), "debhelper (>= 13)"),
                "13\n",
            ),
            // Compat 14 adds the substvar itself. A Build-Depends declares the
            // level only when there is no debian/compat file.
            vec![
                (
                    "debian/control",
                    TestFile::new(control(
                        &binary("foo", "Recommends: ${misc:Depends}\n"),
                        "debhelper-compat (= 14)",
                    )),
                ),
                ("debian/rules", TestFile::new(DH)),
            ],
            // A debputy sequencer suppresses the Tag.
            tree(
                &control(
                    &binary("foo", "Recommends: ${misc:Depends}\n"),
                    "debhelper (>= 13), dh-sequence-debputy",
                ),
                "13\n",
            ),
            // udeb is skipped.
            tree(
                &control(
                    "Package: foo\nArchitecture: all\nPackage-Type: udeb\nSuggests: ${misc:Depends}\nDescription: x\n y\n\n",
                    "debhelper (>= 13)",
                ),
                "13\n",
            ),
        ];
        let fail = vec![
            tree(
                &control(
                    &binary("foo", "Recommends: ${misc:Depends}\n"),
                    "debhelper (>= 13)",
                ),
                "13\n",
            ),
            tree(
                &control(
                    &format!(
                        "{}{}",
                        binary("foo", "Depends: ${misc:Depends}\n"),
                        binary("bar", "Suggests: ${misc:Depends}\n")
                    ),
                    "debhelper (>= 13)",
                ),
                "13\n",
            ),
        ];
        SourceTreeTester::new(WeakDependencyOnMiscDepends, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            &control(
                &binary("foo", "Depends: ${misc:Depends}\n"),
                "debhelper (>= 13)",
            ),
            "13\n",
        ))];
        let fail = vec![SourcePackageCase::from(tree(
            &control(
                &binary("foo", "Suggests: ${misc:Depends}\n"),
                "debhelper (>= 13)",
            ),
            "13\n",
        ))];
        SourcePackageTester::new(WeakDependencyOnMiscDepends, pass, fail).test_and_snapshot();
    }
}
