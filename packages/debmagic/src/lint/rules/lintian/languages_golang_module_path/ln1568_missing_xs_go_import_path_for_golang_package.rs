use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

pub struct MissingXsGoImportPathForGolangPackage;

declare_rule! {
    /// A Source package builds with Go but its source paragraph has no
    /// `XS-Go-Import-Path`.
    ///
    /// Reported when the source paragraph's `Build-Depends` satisfies
    /// `golang-go | golang-any` and the paragraph does not declare
    /// `XS-Go-Import-Path`. Only `Build-Depends` counts, not
    /// `Build-Depends-Indep` or `Build-Depends-Arch`, as in lintian. The check
    /// on `go.mod` that lintian runs after this one belongs to the other Golang
    /// Rule. Extra is empty and there is no Location. Severity is info and the
    /// Tag is not selected by default, as in lintian.
    MissingXsGoImportPathForGolangPackage,
    code = "LN1568",
    tag = "missing-xs-go-import-path-for-golang-package",
    default_selected = false,
    experimental = false,
}

impl SourceRule for MissingXsGoImportPathForGolangPackage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let source = control.source();
        let builds_with_go = source
            .relation("Build-Depends")
            .is_some_and(|depends| depends.satisfies("golang-go | golang-any"));
        if builds_with_go && !source.declares("XS-Go-Import-Path") {
            ctx.diagnostic(Diagnostic::warn("").with_severity(Severity::Info));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(source_fields: &str) -> String {
        format!(
            "Source: example\n\
             Maintainer: Example <ex@example.com>\n\
             Standards-Version: 4.7.2\n\
             {source_fields}\
             \n\
             Package: example\n\
             Architecture: any\n\
             Description: example\n extra\n"
        )
    }

    fn tree(source_fields: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/control", TestFile::new(control(source_fields)))]
    }

    fn package(source_fields: &str) -> SourcePackageCase {
        let dsc = Dsc::parse(
            "Source: example\nBinary: example\nArchitecture: any\nVersion: 1.0-1\n\
             Maintainer: Example <ex@example.com>\nFiles:\n",
        )
        .expect("valid dsc");
        SourcePackageCase::from(tree(source_fields)).dsc(dsc)
    }

    #[test]
    fn tree_cases() {
        let pass = vec![
            tree(""),
            tree("Build-Depends: debhelper-compat (= 13)\n"),
            tree("Build-Depends: golang-go\nXS-Go-Import-Path: example.org/x\n"),
            tree("Build-Depends: golang-any\nXS-Go-Import-Path: example.org/x\n"),
            // Only Build-Depends counts; a declared but empty field counts as declared.
            tree("Build-Depends-Indep: golang-go\n"),
            tree("Build-Depends: golang-go\nXS-Go-Import-Path:\n"),
        ];
        let fail = vec![
            tree("Build-Depends: golang-go\n"),
            tree("Build-Depends: debhelper-compat (= 13), golang-any\n"),
        ];
        SourceTreeTester::new(MissingXsGoImportPathForGolangPackage, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn package_cases() {
        let pass = vec![
            package("Build-Depends: debhelper-compat (= 13)\n"),
            package("Build-Depends: golang-go\nXS-Go-Import-Path: example.org/x\n"),
        ];
        let fail = vec![package("Build-Depends: golang-go\n")];
        SourcePackageTester::new(MissingXsGoImportPathForGolangPackage, pass, fail)
            .test_and_snapshot();
    }
}
