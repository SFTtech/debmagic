use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::{GroupContext, GroupInstallable};
use crate::lint::rule::GroupRule;

pub struct DependencyIsNotMultiArchified;

declare_rule! {
    /// A Multi-Arch `same` installable strongly depends on an installable of
    /// the same source package that is neither Multi-Arch `same` nor
    /// `foreign`. The extra is `<package> depends on <dependency>
    /// (multi-arch: <value>)`, the value being `no` when the dependency has
    /// no `Multi-Arch` field.
    ///
    /// A Group Rule: it needs the sibling installables (the `.deb` files named
    /// in the `.changes`, or given beside it) and the Group's `.dsc`, so it
    /// does not run on a Group without a `.dsc`. Only Pre-Depends and Depends
    /// count, udebs are not looked at as the depending package, and the
    /// Diagnostic attaches to the Source package, as in lintian.
    DependencyIsNotMultiArchified,
    code = "LN1501",
    tag = "dependency-is-not-multi-archified",
    default_selected = true,
    experimental = false,
}

/// `Multi-Arch` as lintian reads it: `no` when absent or empty.
fn multi_arch_or_no<'a>(installable: &GroupInstallable<'a>) -> &'a str {
    match installable.multi_arch() {
        "" => "no",
        value => value,
    }
}

impl GroupRule for DependencyIsNotMultiArchified {
    fn run(&self, ctx: &mut GroupContext<'_>) {
        let installables = ctx.installables();
        for installable in installables.iter().filter(|i| !i.is_udeb()) {
            if multi_arch_or_no(installable) != "same" {
                continue;
            }
            for dependency in ctx.direct_dependencies(installable) {
                let multi_arch = multi_arch_or_no(&dependency);
                if multi_arch != "same" && multi_arch != "foreign" {
                    ctx.diagnostic(Diagnostic::warn(format!(
                        "{} depends on {} (multi-arch: {multi_arch})",
                        installable.name(),
                        dependency.name()
                    )));
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
            // Both Multi-Arch same, or the dependency is foreign.
            GroupCase::new(DSC)
                .installable("a", "Multi-Arch: same\nDepends: b\n")
                .installable("b", "Multi-Arch: same\n"),
            GroupCase::new(DSC)
                .installable("a", "Multi-Arch: same\nDepends: b\n")
                .installable("b", "Multi-Arch: foreign\n"),
            // The depending package is not Multi-Arch same.
            GroupCase::new(DSC)
                .installable("a", "Depends: b\n")
                .installable("b", ""),
            GroupCase::new(DSC)
                .installable("a", "Multi-Arch: foreign\nDepends: b\n")
                .installable("b", "Multi-Arch: allowed\n"),
            // A weak relation, and a package outside the Group.
            GroupCase::new(DSC)
                .installable("a", "Multi-Arch: same\nRecommends: b\nDepends: outside\n")
                .installable("b", ""),
            // A udeb depending is not looked at.
            GroupCase::new(DSC)
                .udeb("a-udeb", "Multi-Arch: same\nDepends: b\n")
                .installable("b", ""),
        ];
        let fail = vec![
            GroupCase::new(DSC)
                .installable("a", "Multi-Arch: same\nDepends: b\n")
                .installable("b", ""),
            GroupCase::new(DSC)
                .installable("a", "Multi-Arch: same\nPre-Depends: b (>= 1)\n")
                .installable("b", "Multi-Arch: allowed\n"),
            // Each offending dependency is one finding; a udeb dependency counts.
            GroupCase::new(DSC)
                .installable("a", "Multi-Arch: same\nDepends: b, c, d, e-udeb\n")
                .installable("b", "Multi-Arch: no\n")
                .installable("c", "Multi-Arch: same\n")
                .installable("d", "")
                .udeb("e-udeb", "Multi-Arch: foreign\n"),
        ];
        GroupTester::new(DependencyIsNotMultiArchified, pass, fail).test_and_snapshot();
    }
}
