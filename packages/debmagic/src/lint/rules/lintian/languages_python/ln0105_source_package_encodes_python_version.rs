use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::python_version::encodes_python_version;

pub struct SourcePackageEncodesPythonVersion;

declare_rule! {
    /// The source package name starts with `python` plus a digit and a hyphen.
    ///
    /// `python3-defaults` is the exception. The name is the `Source` field,
    /// or the first changelog entry when that field is absent. Extra is empty.
    SourcePackageEncodesPythonVersion,
    code = "LN0105",
    tag = "source-package-encodes-python-version",
    default_selected = true,
    experimental = false,
}

impl SourceRule for SourcePackageEncodesPythonVersion {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let name = source_name(ctx);
        if encodes_python_version(&name) {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

fn source_name(ctx: &SourceContext<'_>) -> String {
    if let Some(name) = ctx
        .debian_control()
        .and_then(|control| control.source().get("Source"))
        .filter(|name| !name.is_empty())
    {
        return name.to_string();
    }
    ctx.debian_changelog()
        .and_then(|changelog| changelog.entries().first())
        .map(|entry| entry.source().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(source: &str) -> String {
        format!(
            "Source: {source}\n\
             Maintainer: Example <ex@example.com>\n\
             Standards-Version: 4.7.2\n\
             \n\
             Package: {source}\n\
             Architecture: all\n\
             Description: example\n extra\n"
        )
    }

    fn tree(source: &str) -> Vec<(String, TestFile)> {
        vec![(
            "debian/control".to_string(),
            TestFile::from(control(source)),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            tree("example"),
            tree("python-foo"),
            tree("python3-defaults"),
        ];
        let fail = vec![tree("python3-foo"), tree("python2-foo")];
        SourceTreeTester::new(SourcePackageEncodesPythonVersion, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree("example"))];
        let fail = vec![SourcePackageCase::from(tree("python3-foo"))];
        SourcePackageTester::new(SourcePackageEncodesPythonVersion, pass, fail).test_and_snapshot();
    }
}
