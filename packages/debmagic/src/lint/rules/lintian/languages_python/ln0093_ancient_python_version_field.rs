use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct AncientPythonVersionField;

declare_rule! {
    /// A Python version field names a version at or below the ancient threshold.
    ///
    /// Reads `X-Python-Version`, `XS-Python-Version`, and `X-Python3-Version`
    /// on the source paragraph, in that order. The threshold is lintian
    /// 2.139.0 `data/python/versions`: Python 2 at 2.7, Python 3 at 3.5. The
    /// version is the `N.M` at the end of the field (`\b(([23]).\d+)$`). Extra
    /// is the field name and that version. The same thresholds make the
    /// `old-python-version-field` branch unreachable.
    AncientPythonVersionField,
    code = "LN0093",
    tag = "ancient-python-version-field",
    default_selected = true,
    experimental = false,
}

const FIELDS: &[&str] = &["X-Python-Version", "XS-Python-Version", "X-Python3-Version"];

/// `(major, ancient)` from lintian 2.139.0 `data/python/versions`.
const ANCIENT: &[(&str, &str)] = &[("2", "2.7"), ("3", "3.5")];

static VERSION_TAIL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(([23])\.\d+)$").expect("valid regex"));

impl SourceRule for AncientPythonVersionField {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let source = control.source();
        let messages: Vec<String> = FIELDS
            .iter()
            .filter_map(|field| {
                let value = source.get(field)?;
                let caps = VERSION_TAIL.captures(value)?;
                let version = caps.get(1)?.as_str();
                let major = caps.get(2)?.as_str();
                let ancient = ANCIENT
                    .iter()
                    .find(|(python, _)| *python == major)
                    .map(|(_, threshold)| *threshold)?;
                version_lte(version, ancient).then(|| format!("{field} {version}"))
            })
            .collect();
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
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
            tree("X-Python3-Version: >= 3.6\n"),
            tree("X-Python3-Version: >= 3.10\n"),
            tree("X-Python3-Version: all\n"),
            tree("X-Python-Version: >= 2.8\n"),
            tree("X-Python3-Version: current\n"),
        ];
        let fail = vec![
            tree("XS-Python-Version: >= 2.7\nX-Python3-Version: >= 3.4\n"),
            tree("X-Python-Version: 2.6\n"),
            tree("X-Python3-Version: 3.5\n"),
        ];
        SourceTreeTester::new(AncientPythonVersionField, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree("X-Python3-Version: 3.6\n"))];
        let fail = vec![SourcePackageCase::from(tree("X-Python3-Version: 3.4\n"))];
        SourcePackageTester::new(AncientPythonVersionField, pass, fail).test_and_snapshot();
    }

    #[test]
    fn three_ten_is_newer_than_the_ancient_threshold() {
        assert!(!version_lte("3.10", "3.5"));
        assert!(version_lte("3.5", "3.5"));
        assert!(version_lte("2.7", "2.7"));
        assert!(version_lte("2.6", "2.7"));
    }
}
