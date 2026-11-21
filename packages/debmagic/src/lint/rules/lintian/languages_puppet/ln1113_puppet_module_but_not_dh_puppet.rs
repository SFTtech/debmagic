use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct PuppetModuleButNotDhPuppet;

declare_rule! {
    /// A Puppet module package does not build with `dh-puppet`.
    ///
    /// The source name (the Source field of the source paragraph of
    /// `debian/control`) starts with `puppet-module`. The Build-Depends,
    /// Build-Depends-Indep and Build-Depends-Arch relations, taken together,
    /// neither satisfy `dh-puppet` nor `dh-sequence-puppet`. Extra and Location
    /// are empty. Without a source name or a Debian control, nothing is
    /// reported.
    PuppetModuleButNotDhPuppet,
    code = "LN1113",
    tag = "puppet-module-but-not-dh-puppet",
    default_selected = true,
    experimental = false,
}

const BUILD_DEPENDS_ALL: &[&str] = &["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"];

impl SourceRule for PuppetModuleButNotDhPuppet {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(source) = ctx.debian_control().map(|control| control.source()) else {
            return;
        };
        let Some(name) = source.get("Source") else {
            return;
        };
        if !name.starts_with("puppet-module") {
            return;
        }
        let build_all = source.relations_and(BUILD_DEPENDS_ALL);
        if build_all.satisfies("dh-puppet") || build_all.satisfies("dh-sequence-puppet") {
            return;
        }
        ctx.diagnostic(Diagnostic::warn(""));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(source: &str, build: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: {source}\nStandards-Version: 4.7.2\n{build}\n\
                 Package: {source}\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn source() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            // Not a Puppet module.
            control("example", "Build-Depends: debhelper-compat (= 14)\n"),
            control("example", ""),
            // The name must start with puppet-module.
            control("foo-puppet-module", ""),
            // Puppet module built with dh-puppet, through any of the three fields.
            control(
                "puppet-module-foo",
                "Build-Depends: debhelper-compat (= 14), dh-puppet\n",
            ),
            control(
                "puppet-module-foo",
                "Build-Depends: debhelper-compat (= 14)\nBuild-Depends-Indep: dh-puppet\n",
            ),
            control(
                "puppet-module-foo",
                "Build-Depends: debhelper-compat (= 14)\nBuild-Depends-Arch: dh-puppet (>= 1)\n",
            ),
            control(
                "puppet-module-foo",
                "Build-Depends: debhelper-compat (= 14), dh-sequence-puppet\n",
            ),
        ];
        let fail = vec![
            control(
                "puppet-module-foo",
                "Build-Depends: debhelper-compat (= 14)\n",
            ),
            control("puppet-module-foo", ""),
            // An alternative does not satisfy the prerequisite.
            control(
                "puppet-module-foo",
                "Build-Depends: dh-puppet | debhelper\n",
            ),
            // Build-Depends-All is the three fields, not Build-Conflicts.
            control(
                "puppet-module-foo",
                "Build-Depends: debhelper-compat (= 14)\nBuild-Conflicts: dh-puppet\n",
            ),
        ];
        SourceTreeTester::new(PuppetModuleButNotDhPuppet, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(control(
            "puppet-module-foo",
            "Build-Depends: dh-puppet\n",
        ))];
        let fail = vec![SourcePackageCase::from(control(
            "puppet-module-foo",
            "Build-Depends: debhelper-compat (= 14)\n",
        ))];
        SourcePackageTester::new(PuppetModuleButNotDhPuppet, pass, fail).test_and_snapshot();
    }
}
