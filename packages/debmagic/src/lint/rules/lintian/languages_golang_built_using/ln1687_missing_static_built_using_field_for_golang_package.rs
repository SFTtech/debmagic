use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

pub struct MissingStaticBuiltUsingFieldForGolangPackage;

declare_rule! {
    /// The Source package builds with Go (`Build-Depends` satisfies
    /// `golang-go | golang-any`), and a non-`all` binary package stanza has no
    /// `Static-Built-Using` field containing `${misc:Static-Built-Using}`
    /// while the debhelper compat level is below 14. Below 14, debhelper
    /// does not add the implicit `Built-Using` for static Go binaries.
    /// Reported at info severity, at the stanza's `Package` line in
    /// `debian/control`, with extra `(in section for <binary>)`.
    ///
    /// Skipped when no debhelper compat level is declared (lintian's undef
    /// level). `Architecture: all` stanzas, and a missing `Architecture`
    /// counts as not `all`, as in lintian.
    MissingStaticBuiltUsingFieldForGolangPackage,
    code = "LN1687",
    tag = "missing-static-built-using-field-for-golang-package",
    default_selected = false,
    experimental = false,
}

/// The debhelper compat level from which `dh` adds the implicit `Built-Using`.
const MIN_DH_COMPAT_IMPLICIT_BUILT_USING: u32 = 14;

impl SourceRule for MissingStaticBuiltUsingFieldForGolangPackage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let builds_with_go = control
            .source()
            .relation("Build-Depends")
            .is_some_and(|relation| relation.satisfies("golang-go | golang-any"));
        if !builds_with_go {
            return;
        }
        let candidates: Vec<(String, Option<u32>)> = control
            .installables()
            .iter()
            .filter(|installable| {
                let fields = installable.fields();
                let static_built_using = fields
                    .unfolded_value("Static-Built-Using")
                    .unwrap_or_default();
                !static_built_using.contains("${misc:Static-Built-Using}")
                    && fields.get("Architecture") != Some("all")
            })
            .map(|installable| {
                (
                    installable.name().to_string(),
                    installable.fields().position("Package"),
                )
            })
            .collect();
        if candidates.is_empty() {
            return;
        }
        let Some(level) = ctx.debhelper().and_then(|facts| facts.declared_level()) else {
            return;
        };
        if level >= MIN_DH_COMPAT_IMPLICIT_BUILT_USING {
            return;
        }
        for (name, line) in candidates {
            ctx.diagnostic(
                Diagnostic::warn(format!("(in section for {name})"))
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path: PathBuf::from("debian/control"),
                        line,
                        column: None,
                    }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    /// The compat level is only declared where `debian/rules` runs a dh
    /// command, so a rules file with `dh $@` is included.
    fn tree(build_depends: &str, binary: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            (
                "debian/control",
                TestFile::new(format!(
                    "Source: example\n\
                     Maintainer: Example <ex@example.com>\n\
                     Standards-Version: 4.7.2\n\
                     Build-Depends: {build_depends}\n\
                     \n\
                     Package: example\n\
                     {binary}\
                     Description: example\n extra\n"
                )),
            ),
            (
                "debian/rules",
                TestFile::new("#!/usr/bin/make -f\n%:\n\tdh $@\n"),
            ),
        ]
    }

    const GO: &str = "debhelper-compat (= 13), golang-go";
    const BIN: &str = "Architecture: any\n";

    #[test]
    fn test() {
        let pass = vec![
            // Not a Go package.
            tree("debhelper-compat (= 13)", BIN),
            // Architecture all.
            tree(GO, "Architecture: all\n"),
            // Field present with the substvar.
            tree(
                GO,
                "Architecture: any\nStatic-Built-Using: ${misc:Static-Built-Using}\n",
            ),
            // No debhelper compat level declared.
            tree("golang-go", BIN),
            // Compat 14 adds Built-Using itself.
            tree("debhelper-compat (= 14), golang-go", BIN),
        ];
        let fail = vec![
            tree(GO, BIN),
            tree(GO, "Architecture: any\nStatic-Built-Using: foo\n"),
            tree("debhelper-compat (= 13), golang-go | golang-any", BIN),
        ];
        SourceTreeTester::new(MissingStaticBuiltUsingFieldForGolangPackage, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(GO, "Architecture: all\n"))];
        let fail = vec![SourcePackageCase::from(tree(GO, BIN))];
        SourcePackageTester::new(MissingStaticBuiltUsingFieldForGolangPackage, pass, fail)
            .test_and_snapshot();
    }
}
