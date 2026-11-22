use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::data;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::{GroupContext, GroupInstallable};
use crate::lint::rule::{GroupRule, Severity};

pub struct DebugPackageForMultiArchSamePkgNotCoinstallable;

declare_rule! {
    /// A debug package that is not Multi-Arch `same` depends on a
    /// non-debug installable of the same source package that is
    /// Multi-Arch `same`, so the debug symbols of every architecture variant
    /// cannot be installed together. The extra is
    /// `<debug package> => <dependency>`.
    ///
    /// A debug package is an installable (not a udeb) in a `debug` section
    /// (`debug`, or `<area>/debug`). Only Pre-Depends and Depends count. The
    /// installables named by the `common/dbg-pkg` table (`libc<N>-dbg`,
    /// `python*-dbg`) are exempt.
    ///
    /// Info severity, not selected by default, as the Tag is.
    ///
    /// A Group Rule: it needs the sibling installables (the `.deb` files named
    /// in the `.changes`, or given beside it) and the Group's `.dsc`, so it
    /// does not run on a Group without a `.dsc`. The Diagnostic attaches to
    /// the Source package, as in lintian.
    DebugPackageForMultiArchSamePkgNotCoinstallable,
    code = "LN1505",
    tag = "debug-package-for-multi-arch-same-pkg-not-coinstallable",
    default_selected = false,
    experimental = false,
}

/// Patterns of `common/dbg-pkg`, compiled once.
static KNOWN_DBG_PACKAGES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    data::lines(include_str!("../../../data/common/dbg-pkg"))
        .map(|pattern| Regex::new(pattern).expect("valid regex in common/dbg-pkg"))
        .collect()
});

fn multi_arch_or_no<'a>(installable: &GroupInstallable<'a>) -> &'a str {
    match installable.multi_arch() {
        "" => "no",
        value => value,
    }
}

/// The `Section` is `debug`, or ends in `/debug`; `none` when absent.
fn is_debug_section(installable: &GroupInstallable<'_>) -> bool {
    let section = installable.fields().get("Section").map_or("", str::trim);
    section == "debug" || section.ends_with("/debug")
}

impl GroupRule for DebugPackageForMultiArchSamePkgNotCoinstallable {
    fn run(&self, ctx: &mut GroupContext<'_>) {
        let installables = ctx.installables();
        for installable in installables.iter().filter(|i| !i.is_udeb()) {
            if multi_arch_or_no(installable) == "same" || !is_debug_section(installable) {
                continue;
            }
            for dependency in ctx.direct_dependencies(installable) {
                if multi_arch_or_no(&dependency) == "same"
                    && !is_debug_section(&dependency)
                    && !KNOWN_DBG_PACKAGES
                        .iter()
                        .any(|pattern| pattern.is_match(installable.name()))
                {
                    ctx.diagnostic(
                        Diagnostic::warn(format!(
                            "{} => {}",
                            installable.name(),
                            dependency.name()
                        ))
                        .with_severity(Severity::Info),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{GroupCase, GroupTester};

    use super::*;

    const DSC: &str = "Source: example\nVersion: 1.0\n";

    #[test]
    fn group() {
        let pass = vec![
            GroupCase::new(DSC),
            // The debug package is Multi-Arch same itself.
            GroupCase::new(DSC)
                .installable("a-dbg", "Section: debug\nMulti-Arch: same\nDepends: a\n")
                .installable("a", "Multi-Arch: same\n"),
            // The dependency is not Multi-Arch same, or is a debug package.
            GroupCase::new(DSC)
                .installable("a-dbg", "Section: debug\nDepends: a\n")
                .installable("a", "Multi-Arch: foreign\n"),
            GroupCase::new(DSC)
                .installable("a-dbg", "Section: debug\nDepends: b-dbg\n")
                .installable("b-dbg", "Section: libs/debug\nMulti-Arch: same\n"),
            // Not a debug package.
            GroupCase::new(DSC)
                .installable("a-tools", "Section: utils\nDepends: a\n")
                .installable("a", "Multi-Arch: same\n"),
            GroupCase::new(DSC)
                .installable("a-dbg", "Section: debugging\nDepends: a\n")
                .installable("a", "Multi-Arch: same\n"),
            // Known debug packages.
            GroupCase::new(DSC)
                .installable("libc6-dbg", "Section: debug\nDepends: a\n")
                .installable("a", "Multi-Arch: same\n"),
            GroupCase::new(DSC)
                .installable("python3-foo-dbg", "Section: debug\nDepends: a\n")
                .installable("a", "Multi-Arch: same\n"),
            // A weak relation, a udeb.
            GroupCase::new(DSC)
                .installable("a-dbg", "Section: debug\nRecommends: a\n")
                .installable("a", "Multi-Arch: same\n"),
            GroupCase::new(DSC)
                .udeb("a-dbg", "Section: debug\nDepends: a\n")
                .installable("a", "Multi-Arch: same\n"),
        ];
        let fail = vec![
            GroupCase::new(DSC)
                .installable("pkg-dbg", "Section: debug\nDepends: pkg\n")
                .installable("pkg", "Multi-Arch: same\n"),
            GroupCase::new(DSC)
                .installable(
                    "pkg-dbg",
                    "Section: libs/debug\nMulti-Arch: no\nPre-Depends: pkg (= 1.0)\n",
                )
                .installable("pkg", "Multi-Arch: same\nSection: libs\n"),
            GroupCase::new(DSC)
                .installable("x-dbg", "Section: debug\nDepends: one, two, three\n")
                .installable("one", "Multi-Arch: same\n")
                .installable("two", "Multi-Arch: same\n")
                .installable("three", "Multi-Arch: foreign\n"),
        ];
        GroupTester::new(DebugPackageForMultiArchSamePkgNotCoinstallable, pass, fail)
            .test_and_snapshot();
    }
}
