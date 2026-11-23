use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::python_version;

pub struct OldPythonVersionField;

declare_rule! {
    /// A Python version field names a version above the ancient threshold and
    /// at or below the old threshold (3.12 and 3.13 in lintian 2.144.0
    /// `data/python/versions`). Such a version is satisfied by the current
    /// stable Debian release and may be unnecessary. Reported at pedantic
    /// severity, with the field name and version as the extra.
    ///
    /// Reads `X-Python3-Version` on the source paragraph. The version is the
    /// `3.N` at the end of the field (`\b(3\.\d+)$`), as in lintian. Versions
    /// at or below the ancient threshold are `ancient-python-version-field`
    /// (LN0093).
    OldPythonVersionField,
    code = "LN1682",
    tag = "old-python-version-field",
    default_selected = false,
    experimental = false,
}

static VERSION_TAIL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(3\.\d+)$").expect("valid regex"));

impl SourceRule for OldPythonVersionField {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let (Some(ancient), Some(old)) = (
            python_version::ancient_threshold(),
            python_version::old_threshold(),
        ) else {
            return;
        };
        let messages: Vec<String> = python_version::field_values(control)
            .into_iter()
            .filter_map(|(field, value)| {
                let caps = VERSION_TAIL.captures(value)?;
                let version = caps.get(1)?.as_str();
                let in_range = !version_lte(version, ancient) && version_lte(version, old);
                in_range.then(|| format!("{field} {version}"))
            })
            .collect();
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message).with_severity(Severity::Pedantic));
        }
    }
}

/// Component-wise numeric compare for the `N.M` versions this Rule captures.
fn version_lte(left: &str, right: &str) -> bool {
    let parse = |version: &str| -> Option<(u64, u64)> {
        let (major, minor) = version.split_once('.')?;
        Some((major.parse().ok()?, minor.parse().ok()?))
    };
    match (parse(left), parse(right)) {
        (Some(left), Some(right)) => left <= right,
        _ => false,
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
            tree("X-Python3-Version: >= 3.14\n"),
            tree("X-Python3-Version: 3.14\n"),
            tree("X-Python3-Version: all\n"),
            tree("X-Python3-Version: current\n"),
            // At or below the ancient threshold is LN0093, not this Tag.
            tree("X-Python3-Version: >= 3.11\n"),
            tree("X-Python3-Version: 3.5\n"),
            // Only X-Python3-Version is read.
            tree("X-Python-Version: 3.12\n"),
        ];
        let fail = vec![
            tree("X-Python3-Version: 3.12\n"),
            tree("X-Python3-Version: >= 3.12\n"),
            tree("X-Python3-Version: 3.13\n"),
        ];
        SourceTreeTester::new(OldPythonVersionField, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree("X-Python3-Version: 3.14\n"))];
        let fail = vec![SourcePackageCase::from(tree("X-Python3-Version: 3.13\n"))];
        SourcePackageTester::new(OldPythonVersionField, pass, fail).test_and_snapshot();
    }

    #[test]
    fn range_bounds() {
        assert!(version_lte("3.11", "3.11"));
        assert!(!version_lte("3.12", "3.11"));
        assert!(version_lte("3.13", "3.13"));
        assert!(!version_lte("3.14", "3.13"));
    }
}
