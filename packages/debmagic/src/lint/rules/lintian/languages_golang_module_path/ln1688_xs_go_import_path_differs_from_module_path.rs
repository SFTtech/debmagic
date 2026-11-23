use crate::declare_rule;
use crate::lint::context::{SourceContext, follow_symlinks};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

pub struct XsGoImportPathDiffersFromModulePath;

declare_rule! {
    /// The Source package builds with Go (`Build-Depends` satisfies
    /// `golang-go | golang-any`), declares `XS-Go-Import-Path`, and its
    /// `go.mod` names a different module path. With module-aware builds the
    /// mismatch can break dependency resolution. Reported at info severity
    /// with no Location and the extra `<XS-Go-Import-Path> vs <module>`.
    ///
    /// The import path is the first comma-separated element, with trailing
    /// whitespace removed. The module path is the token after the first
    /// `module` line (lintian's `^\s*module\s+"?(\S+)"?`): a quote directly
    /// after the keyword is dropped, a closing quote is kept, and a missing
    /// line gives an empty module path. A `go.mod` that is not an open file
    /// is skipped. A missing `XS-Go-Import-Path` is LN1568
    /// (`missing-xs-go-import-path-for-golang-package`).
    XsGoImportPathDiffersFromModulePath,
    code = "LN1688",
    tag = "xs-go-import-path-differs-from-module-path",
    default_selected = false,
    experimental = false,
}

impl SourceRule for XsGoImportPathDiffersFromModulePath {
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
        let Some(declared) = control.source().unfolded_value("XS-Go-Import-Path") else {
            return;
        };
        let import_path = declared
            .split(',')
            .next()
            .unwrap_or("")
            .trim_end()
            .to_string();
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(entry) = files.get("go.mod") else {
            return;
        };
        let Some(target) = follow_symlinks(files, entry) else {
            return;
        };
        let Ok(bytes) = files.read(target) else {
            return;
        };
        let module_path = module_path(&bytes);
        if module_path != import_path {
            ctx.diagnostic(
                Diagnostic::warn(format!("{import_path} vs {module_path}"))
                    .with_severity(Severity::Info),
            );
        }
    }
}

/// The module path of the first `module` line of a `go.mod`, as lintian's
/// regex reads it. Empty when there is none.
fn module_path(bytes: &[u8]) -> String {
    for line in bytes.split(|byte| *byte == b'\n') {
        let line = String::from_utf8_lossy(line);
        let Some(rest) = line.trim_start().strip_prefix("module") else {
            continue;
        };
        if !rest.starts_with(char::is_whitespace) {
            continue;
        }
        let rest = rest.trim_start();
        // `"?` takes a leading quote only when a token follows it.
        let token = match rest.strip_prefix('"') {
            Some(after) if !after.is_empty() => after,
            _ => rest,
        };
        let token = token.split(char::is_whitespace).next().unwrap_or("");
        if !token.is_empty() {
            return token.to_string();
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn tree(xs: Option<&str>, gomod: Option<&str>) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\n\
                 Maintainer: Example <ex@example.com>\n\
                 Standards-Version: 4.7.2\n\
                 Build-Depends: debhelper-compat (= 13), golang-go\n\
                 {}\
                 \n\
                 Package: example\n\
                 Architecture: any\n\
                 Description: example\n extra\n",
                xs.map(|xs| format!("XS-Go-Import-Path: {xs}\n"))
                    .unwrap_or_default()
            )),
        )];
        if let Some(gomod) = gomod {
            files.push(("go.mod", TestFile::new(gomod)));
        }
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(
                Some("github.com/x/y"),
                Some("module github.com/x/y\n\ngo 1.21\n"),
            ),
            tree(
                Some("github.com/x/y, github.com/x/z"),
                Some("module github.com/x/y\n"),
            ),
            tree(Some("github.com/x/y"), None),
            tree(None, Some("module github.com/x/z\n")),
        ];
        let fail = vec![
            tree(Some("github.com/x/y"), Some("module github.com/x/z\n")),
            tree(Some("github.com/x/y"), Some("go 1.21\n")),
            tree(Some("github.com/x/y"), Some("module \"github.com/x/y\"\n")),
        ];
        SourceTreeTester::new(XsGoImportPathDiffersFromModulePath, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            Some("github.com/x/y"),
            Some("module github.com/x/y\n"),
        ))];
        let fail = vec![SourcePackageCase::from(tree(
            Some("github.com/x/y"),
            Some("module github.com/x/z\n"),
        ))];
        SourcePackageTester::new(XsGoImportPathDiffersFromModulePath, pass, fail)
            .test_and_snapshot();
    }
}
