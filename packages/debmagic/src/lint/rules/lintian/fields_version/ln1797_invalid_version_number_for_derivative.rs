use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::derivative::{self, Pattern};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::ln0925_bad_version_number::is_valid_version;
use super::{is_native, source_version};

pub struct InvalidVersionNumberForDerivative;

declare_rule! {
    /// The Version of a non-native source package does not match a derivative
    /// pattern of lintian's `data/fields/derivative-versions`. The extra is the
    /// version and the explanation, as `version (explanation)`; one hint for
    /// each pattern it does not match.
    ///
    /// Only a valid Version is checked (see `bad-version-number`), and a native
    /// package is not. The Version is the Dsc's, or on a Source tree the newest
    /// `debian/changelog` heading, as for the other `fields/version` Rules.
    ///
    /// Debian's table has one pattern, `.`, which matches any non-empty
    /// version. A valid Version is never empty, so Debian never gets this Tag:
    /// the Rule reports nothing.
    InvalidVersionNumberForDerivative,
    code = "LN1797",
    tag = "invalid-version-number-for-derivative",
    default_selected = true,
    experimental = false,
}

impl SourceRule for InvalidVersionNumberForDerivative {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(version) = source_version(ctx) else {
            return;
        };
        if !is_valid_version(&version) || is_native(ctx) {
            return;
        }
        let reports = invalid_versions(&version, derivative::versions());
        for extra in reports {
            ctx.diagnostic(Diagnostic::error(extra));
        }
    }
}

/// One `version (explanation)` for each pattern the version does not match,
/// in table order.
fn invalid_versions(version: &str, table: &[Pattern]) -> Vec<String> {
    table
        .iter()
        .filter(|pattern| !pattern.regex.is_match(version))
        .map(|pattern| format!("{version} ({})", pattern.explanation))
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn entry(version: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn tree(version: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new("Source: example\n")),
            ("debian/changelog", TestFile::new(entry(version))),
        ]
    }

    fn dsc(version: &str) -> Dsc {
        Dsc::parse(&format!(
            "Format: 3.0 (quilt)\nSource: example\nVersion: {version}\n"
        ))
        .expect("Dsc fixture")
    }

    fn package(version: &str) -> SourcePackageCase {
        SourcePackageCase::from(Vec::<(&str, TestFile)>::new()).dsc(dsc(version))
    }

    #[test]
    fn source_tree() {
        // Debian's table matches every non-empty version, so nothing is reported.
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            tree("1.0-1"),
            tree("1.0"),
            tree("1:2.0~rc1-1ubuntu1"),
        ];
        let fail = Vec::<Vec<(&str, TestFile)>>::new();
        SourceTreeTester::new(InvalidVersionNumberForDerivative, pass, fail).test();
    }

    #[test]
    fn source_package() {
        let pass = vec![
            package("1.0-1"),
            package("1.0"),
            package("1:2.0~rc1-1.1"),
            // Not a valid version, so it is left to `bad-version-number`.
            package("1.0?-1"),
        ];
        let fail = Vec::<SourcePackageCase>::new();
        SourcePackageTester::new(InvalidVersionNumberForDerivative, pass, fail).test();
    }

    #[test]
    fn reports_each_pattern_a_version_breaks() {
        let table = vec![Pattern {
            regex: regex::Regex::new(r"-1$").expect("test regex"),
            explanation: "needs the -1 revision".to_string(),
        }];
        assert_eq!(invalid_versions("1.0-1", &table), Vec::<String>::new());
        assert_eq!(
            invalid_versions("1.0-2", &table),
            vec!["1.0-2 (needs the -1 revision)".to_string()]
        );
    }
}
