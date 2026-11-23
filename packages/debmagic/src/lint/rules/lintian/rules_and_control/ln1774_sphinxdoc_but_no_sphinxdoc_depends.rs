use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct SphinxdocButNoSphinxdocDepends;

declare_rule! {
    /// `debian/rules` uses the `sphinxdoc` helper but no installable depends on
    /// `${sphinxdoc:Depends}`.
    ///
    /// Scans `debian/rules` the way lintian's `debhelper` Check does: a
    /// `dh_sphinxdoc` command (anywhere, make conditionals included) or a
    /// `sphinxdoc` add-on of a `dh` line marks the helper as used. Reported
    /// when no installable's `Pre-Depends`, `Depends`, `Recommends`, or
    /// `Suggests` mention `${sphinxdoc:Depends}`; udebs count as well, as in
    /// lintian. Silent when `debian/rules` has a make-variable `dh` line (any
    /// add-on may be in use), when compat is 14 or above (debhelper adds the
    /// substvar itself), and when a build dependency on `dh-sequence-debputy`,
    /// `dh-sequence-zz-debputy`, or `dh-sequence-zz-debputy-rrr` is present.
    /// Only reached after a dh command has been seen; a `Build-Driver` other
    /// than `debian-rules` is not scanned. Extra is empty; the Location is
    /// `debian/rules` without a line. Applies to the Source tree and Source
    /// package.
    SphinxdocButNoSphinxdocDepends,
    code = "LN1774",
    tag = "sphinxdoc-but-no-sphinxdoc-depends",
    default_selected = true,
    experimental = false,
}

/// First compat level at which debhelper adds `${misc:Depends}` itself.
/// lintian's `$dh_no_misc` uses the same level.
const MISC_DEPENDS_AUTO_APPLY: u32 = 14;

const SPHINXDOC_DEPENDS: &str = "${sphinxdoc:Depends}";

const BUILD_DEPENDS: &[&str] = &["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"];

const INSTALLATION: &[&str] = &["Pre-Depends", "Depends", "Recommends", "Suggests"];

const DEBPUTY: &[&str] = &[
    "dh-sequence-debputy",
    "dh-sequence-zz-debputy",
    "dh-sequence-zz-debputy-rrr",
];

impl SourceRule for SphinxdocButNoSphinxdocDepends {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = ctx.debhelper() else {
            return;
        };
        if !facts.uses_debhelper() {
            return;
        }
        // lintian: `$seen{'sphinxdoc'} && !$seen_dh_dynamic`.
        let sphinxdoc_used = facts
            .commands()
            .iter()
            .any(|command| command.name() == "dh_sphinxdoc")
            || facts.addons().iter().any(|addon| addon == "sphinxdoc");
        if !sphinxdoc_used || facts.has_dynamic_addons() {
            return;
        }
        if facts.level() >= MISC_DEPENDS_AUTO_APPLY {
            return;
        }
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let build = control.source().relations_and(BUILD_DEPENDS);
        if DEBPUTY.iter().any(|package| build.satisfies(package)) {
            return;
        }
        // Every installable counts, udebs included.
        let satisfied = control.installables().iter().any(|installable| {
            installable
                .fields()
                .relations_and(INSTALLATION)
                .satisfies(SPHINXDOC_DEPENDS)
        });
        if satisfied {
            return;
        }
        ctx.diagnostic(Diagnostic::warn("").with_location(Location {
            path: PathBuf::from("debian/rules"),
            line: None,
            column: None,
        }));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const DH_PLAIN: &str = "#!/usr/bin/make -f\n%:\n\tdh $@\n";
    const DH_WITH_SPHINXDOC: &str = "#!/usr/bin/make -f\n%:\n\tdh $@ --with sphinxdoc\n";
    const DH_WITH_DASHED: &str = "#!/usr/bin/make -f\n%:\n\tdh $@ --with python3,sphinxdoc\n";
    const DH_COMMAND: &str =
        "#!/usr/bin/make -f\n%:\n\tdh $@\n\noverride_dh_sphinxdoc:\n\tdh_sphinxdoc\n";
    const DH_COMMAND_IN_CONDITIONAL: &str =
        "#!/usr/bin/make -f\n%:\n\tdh $@\n\nifeq (,$(X))\n\tdh_sphinxdoc\nendif\n";
    const DH_COMMENTED: &str = "#!/usr/bin/make -f\n%:\n\tdh $@\n\n#\tdh_sphinxdoc\n";
    const DH_DYNAMIC: &str = "#!/usr/bin/make -f\n%:\n\tdh $@ --with $(ADDONS)\n";

    const BINARY: &str = "Package: example\nArchitecture: all\nDescription: example\n extra\n";
    const OTHER: &str = "Package: other\nArchitecture: all\nDescription: other\n extra\n";

    /// A binary paragraph with `field` added before the description.
    fn binary(field: &str) -> String {
        format!("Package: example\nArchitecture: all\n{field}\nDescription: example\n extra\n")
    }

    fn tree(control: &str, rules: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(control.to_string())),
            ("debian/rules", TestFile::new(rules.to_string())),
        ]
    }

    #[test]
    fn test() {
        let with_substvar = format!(
            "Source: example\n\n{}",
            binary("Depends: ${sphinxdoc:Depends}")
        );
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            // No sphinxdoc helper.
            tree(&format!("Source: example\n\n{BINARY}"), DH_PLAIN),
            // The substvar in any of the four relation fields.
            tree(&with_substvar, DH_WITH_SPHINXDOC),
            tree(
                &format!(
                    "Source: example\n\n{}",
                    binary("Recommends: other, ${sphinxdoc:Depends}")
                ),
                DH_COMMAND,
            ),
            tree(
                &format!(
                    "Source: example\n\n{}",
                    binary("Suggests: ${sphinxdoc:Depends}")
                ),
                DH_WITH_DASHED,
            ),
            tree(
                &format!(
                    "Source: example\n\n{}",
                    binary("Pre-Depends: ${sphinxdoc:Depends}")
                ),
                DH_WITH_SPHINXDOC,
            ),
            // One installable with the substvar is enough.
            tree(
                &format!(
                    "Source: example\n\n{BINARY}\n{}",
                    binary("Depends: ${sphinxdoc:Depends}")
                        .replace("Package: example", "Package: other")
                ),
                DH_WITH_SPHINXDOC,
            ),
            // udebs count too.
            tree(
                "Source: example\n\nPackage: example-udeb\nPackage-Type: udeb\nArchitecture: any\n\
                 Depends: ${sphinxdoc:Depends}\nDescription: udeb\n extra\n",
                DH_WITH_SPHINXDOC,
            ),
            // Compat 14 adds the substvar itself.
            tree(
                &format!("Source: example\nBuild-Depends: debhelper-compat (= 14)\n\n{BINARY}"),
                DH_WITH_SPHINXDOC,
            ),
            // A debputy sequencer build dependency.
            tree(
                &format!("Source: example\nBuild-Depends: dh-sequence-debputy\n\n{BINARY}"),
                DH_WITH_SPHINXDOC,
            ),
            // A make variable in a `dh` line: any add-on may be in use.
            tree(&format!("Source: example\n\n{BINARY}"), DH_DYNAMIC),
            // Not debian-rules: lintian does not scan `debian/rules`.
            tree(
                &format!("Source: example\nBuild-Driver: other\n\n{BINARY}"),
                DH_WITH_SPHINXDOC,
            ),
            // A commented-out helper does not count.
            tree(&format!("Source: example\n\n{BINARY}"), DH_COMMENTED),
        ];
        let fail = vec![
            tree(&format!("Source: example\n\n{BINARY}"), DH_COMMAND),
            tree(&format!("Source: example\n\n{BINARY}"), DH_WITH_SPHINXDOC),
            // Make conditionals do not hide the command.
            tree(
                &format!("Source: example\n\n{BINARY}"),
                DH_COMMAND_IN_CONDITIONAL,
            ),
            // The substvar must be in a relation field lintian reads.
            tree(
                &format!(
                    "Source: example\n\n{}",
                    binary("Breaks: ${sphinxdoc:Depends}")
                ),
                DH_WITH_SPHINXDOC,
            ),
            tree(
                &format!("Source: example\n\n{}", binary("Depends: ${misc:Depends}")),
                DH_WITH_SPHINXDOC,
            ),
            // Other build dependencies do not replace the debputy sequencer.
            tree(
                &format!("Source: example\nBuild-Depends: dh-sequence-python3\n\n{BINARY}"),
                DH_WITH_SPHINXDOC,
            ),
            // Compat 13 does not add it.
            tree(
                &format!("Source: example\nBuild-Depends: debhelper-compat (= 13)\n\n{BINARY}"),
                DH_WITH_SPHINXDOC,
            ),
            // Several installables without the substvar report once.
            tree(
                &format!("Source: example\n\n{BINARY}\n{OTHER}"),
                DH_WITH_SPHINXDOC,
            ),
        ];
        SourceTreeTester::new(SphinxdocButNoSphinxdocDepends, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let control = format!(
            "Source: example\n\n{}",
            binary("Depends: ${sphinxdoc:Depends}")
        );
        let pass = vec![
            SourcePackageCase::from(vec![
                ("debian/control", TestFile::from(control.clone())),
                ("debian/rules", TestFile::new(DH_WITH_SPHINXDOC)),
            ]),
            SourcePackageCase::from(vec![
                (
                    "debian/control",
                    TestFile::from(format!("Source: example\n\n{BINARY}")),
                ),
                ("debian/rules", TestFile::new(DH_PLAIN)),
            ]),
        ];
        let fail = vec![SourcePackageCase::from(vec![
            (
                "debian/control",
                TestFile::from(format!("Source: example\n\n{BINARY}")),
            ),
            ("debian/rules", TestFile::new(DH_WITH_SPHINXDOC)),
        ])];
        SourcePackageTester::new(SphinxdocButNoSphinxdocDepends, pass, fail).test_and_snapshot();
    }
}
