use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

pub struct UselessPydistOverridesFile;

declare_rule! {
    /// The Source package has a `debian/pydist-overrides` file. That file
    /// held Python 2 dependency overrides and is obsolete, so it is useless.
    /// Reported at info severity with an empty extra and no Location, once
    /// per Source package.
    ///
    /// Looks up the path in the patched files (`ctx.files()`). The entry is
    /// reported whatever its type, and a symlink is not followed. The sibling
    /// `debian/py3dist-overrides` is read by LN0819 (`duplicate-p3dist-override`).
    UselessPydistOverridesFile,
    code = "LN1684",
    tag = "useless-pydist-overrides-file",
    default_selected = false,
    experimental = false,
}

impl SourceRule for UselessPydistOverridesFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        if files.get("debian/pydist-overrides").is_some() {
            ctx.diagnostic(Diagnostic::warn("").with_severity(Severity::Info));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn tree(extra: Option<&str>) -> Vec<(String, TestFile)> {
        let mut files = vec![(
            "debian/control".to_string(),
            TestFile::from(
                "Source: example\n\
                 Maintainer: Example <ex@example.com>\n\
                 Standards-Version: 4.7.2\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n",
            ),
        )];
        if let Some(path) = extra {
            files.push((path.to_string(), TestFile::from("pycairo\n")));
        }
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(None),
            tree(Some("debian/py3dist-overrides")),
            tree(Some("debian/pydist-overridesx")),
        ];
        let fail = vec![tree(Some("debian/pydist-overrides"))];
        SourceTreeTester::new(UselessPydistOverridesFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(None))];
        let fail = vec![SourcePackageCase::from(tree(Some(
            "debian/pydist-overrides",
        )))];
        SourcePackageTester::new(UselessPydistOverridesFile, pass, fail).test_and_snapshot();
    }
}
