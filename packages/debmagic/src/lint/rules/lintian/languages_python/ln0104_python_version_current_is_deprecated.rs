use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::python_version::{current_is_deprecated, field_values};

pub struct PythonVersionCurrentIsDeprecated;

declare_rule! {
    /// A Python version field starts with `current`.
    ///
    /// Reads `X-Python-Version`, `XS-Python-Version`, and `X-Python3-Version`
    /// on the source paragraph. Extra is the field name.
    PythonVersionCurrentIsDeprecated,
    code = "LN0104",
    tag = "python-version-current-is-deprecated",
    default_selected = true,
    experimental = false,
}

impl SourceRule for PythonVersionCurrentIsDeprecated {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let fields: Vec<&str> = field_values(control)
            .into_iter()
            .filter(|(_, value)| current_is_deprecated(value))
            .map(|(field, _)| field)
            .collect();
        for field in fields {
            ctx.diagnostic(Diagnostic::warn(field));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(fields: &str) -> String {
        format!(
            "Source: example\n\
             Maintainer: Example <ex@example.com>\n\
             Standards-Version: 4.7.2\n\
             {fields}\n\
             \n\
             Package: example\n\
             Architecture: all\n\
             Description: example\n extra\n"
        )
    }

    fn tree(fields: &str) -> Vec<(String, TestFile)> {
        vec![(
            "debian/control".to_string(),
            TestFile::from(control(fields)),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(""),
            tree("X-Python3-Version: >= 3.6\n"),
            tree("X-Python3-Version: all\n"),
        ];
        let fail = vec![
            tree("X-Python3-Version: current\n"),
            tree("XS-Python-Version: current 2.7\n"),
        ];
        SourceTreeTester::new(PythonVersionCurrentIsDeprecated, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree("X-Python3-Version: >= 3.6\n"))];
        let fail = vec![SourcePackageCase::from(tree(
            "X-Python3-Version: current\n",
        ))];
        SourcePackageTester::new(PythonVersionCurrentIsDeprecated, pass, fail).test_and_snapshot();
    }
}
