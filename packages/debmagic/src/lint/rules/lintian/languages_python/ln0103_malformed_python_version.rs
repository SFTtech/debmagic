use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::python_version::{field_values, is_malformed};

pub struct MalformedPythonVersion;

declare_rule! {
    /// A Python version field is not a version, a range, `current`, or `all`.
    ///
    /// Reads `X-Python-Version`, `XS-Python-Version`, and `X-Python3-Version`
    /// on the source paragraph. One or two comma-separated parts must match
    /// the shapes lintian 2.139.0 accepts. More than two parts are accepted
    /// only when every part is `N.M`. Extra is the field name and its value.
    MalformedPythonVersion,
    code = "LN0103",
    tag = "malformed-python-version",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MalformedPythonVersion {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let messages: Vec<String> = field_values(control)
            .into_iter()
            .filter(|(_, value)| is_malformed(value))
            .map(|(field, value)| format!("{field} {value}"))
            .collect();
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
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
            tree("X-Python-Version: 2.7, 2.6\n"),
            tree("X-Python3-Version: current\n"),
        ];
        let fail = vec![
            tree("XS-Python-Version: current 2.7\nX-Python3-Version: invalid\n"),
            tree("X-Python-Version: 2.7, foo, 3.5\n"),
        ];
        SourceTreeTester::new(MalformedPythonVersion, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree("X-Python3-Version: >= 3.6\n"))];
        let fail = vec![SourcePackageCase::from(tree(
            "X-Python3-Version: invalid\n",
        ))];
        SourcePackageTester::new(MalformedPythonVersion, pass, fail).test_and_snapshot();
    }
}
