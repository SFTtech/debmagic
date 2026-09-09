use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceTreeContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceTreeRule;

pub struct DebianRulesMissingRequiredTarget;

declare_rule! {
    /// Policy 4.9 required targets are missing from `debian/rules`.
    ///
    /// Required targets are build, build-arch, build-indep, binary,
    /// binary-arch, binary-indep, and clean. Extra is the missing target
    /// name. A `%` catch-all, `.DEFAULT`, a `.PHONY` listing, or known
    /// helper-makefile includes can satisfy them; an unknown `include`
    /// suppresses the check. Source-tree inspects `debian/rules`.
    DebianRulesMissingRequiredTarget,
    code = "LN0003",
    tag = "debian-rules-missing-required-target",
    default_selected = true,
    experimental = false,
}

impl SourceTreeRule for DebianRulesMissingRequiredTarget {
    fn run(&self, ctx: &mut SourceTreeContext<'_>) {
        let Some(rules) = ctx.debian_rules() else {
            return;
        };
        let reports: Vec<String> = rules.missing_policy_targets().to_vec();

        for target in reports {
            ctx.diagnostic(Diagnostic::error(target).with_location(Location {
                path: PathBuf::from("debian/rules"),
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::SourceTreeTester;

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    #[test]
    fn test() {
        let pass = vec![
            vec![
                ("debian/control", CONTROL),
                ("debian/rules", "#!/usr/bin/make -f\n\n%:\n\tdh $@\n"),
            ],
            vec![
                ("debian/control", CONTROL),
                (
                    "debian/rules",
                    concat!(
                        "#!/usr/bin/make -f\n",
                        "\n",
                        ".PHONY: binary binary-arch binary-indep build build-arch build-indep clean\n",
                        "binary-arch build build-arch build-indep clean install:\n",
                        "\tdh $@\n",
                        "\n",
                        "binary binary-indep:\n",
                        "\tdh $@\n",
                    ),
                ),
            ],
            vec![
                ("debian/control", CONTROL),
                (
                    "debian/rules",
                    concat!(
                        "#!/usr/bin/make -f\n",
                        "\n",
                        "  TARGETS := build clean binary binary-arch binary-indep build-arch build-indep\n",
                        "\n",
                        "$(TARGETS):\n",
                        "\tdh $@\n",
                    ),
                ),
            ],
            vec![
                ("debian/control", CONTROL),
                (
                    "debian/rules",
                    "#!/usr/bin/make -f\n\n\tinclude debian/rules.mk\n",
                ),
            ],
            vec![("debian/control", CONTROL)],
            vec![("debian/control", CONTROL), ("debian/rules", "")],
        ];
        let fail = vec![
            vec![
                ("debian/control", CONTROL),
                (
                    "debian/rules",
                    concat!(
                        "#!/usr/bin/make -f\n",
                        "build:\n",
                        "binary:\n",
                        "\tinstall -d debian/generic-empty debian/generic-empty/DEBIAN\n",
                        "\tdpkg-gencontrol -pgeneric-empty -Pdebian/generic-empty\n",
                        "\tdpkg --build debian/generic-empty ..\n",
                        "\n",
                        "clean:\n",
                        "\trm -rf debian/generic-empty\n",
                    ),
                ),
            ],
            vec![
                ("debian/control", CONTROL),
                (
                    "debian/rules",
                    concat!(
                        "#!/usr/bin/make -f\n",
                        "\n",
                        "build clean binary binary-arch binary-indep:\n",
                        "\tdh $@\n",
                    ),
                ),
            ],
            vec![
                ("debian/control", CONTROL),
                (
                    "debian/rules",
                    concat!(
                        "#!/usr/bin/make -f\n",
                        "\n",
                        "include /usr/share/javahelper/java-vars.mk\n",
                        "\n",
                        "clean build binary:\n",
                        "\tdh $@\n",
                    ),
                ),
            ],
            vec![
                ("debian/control", CONTROL),
                ("debian/rules", "#!/usr/bin/make -f\n"),
            ],
        ];
        SourceTreeTester::new(DebianRulesMissingRequiredTarget, pass, fail).test_and_snapshot();
    }
}
