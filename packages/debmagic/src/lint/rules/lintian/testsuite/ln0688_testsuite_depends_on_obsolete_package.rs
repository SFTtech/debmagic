use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::sections::{Section, tests_control_sections};

pub struct TestsuiteDependsOnObsoletePackage;

declare_rule! {
    /// A test `Depends` names a package lintian lists as obsolete.
    ///
    /// `Depends` of each paragraph of `debian/tests/control` is split on commas
    /// and `|`. An alternative is reported when it is the first of its
    /// dependency or every alternative is obsolete; a later obsolete
    /// alternative beside a current one is `testsuite-ored-depends-on-obsolete-package`.
    /// Extra is `Depends: `, the alternative as written, and ` => replacement`
    /// when the data file names one. There is no Location: lintian's hint has
    /// no pointer, so the hint carries no `[path:line]`.
    /// Applies to the Source tree and Source package.
    TestsuiteDependsOnObsoletePackage,
    code = "LN0688",
    tag = "testsuite-depends-on-obsolete-package",
    default_selected = true,
    experimental = false,
}

static OBSOLETE: LazyLock<Vec<(String, String)>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/fields/obsolete-packages"))
        .map(|line| {
            let (name, replacement) = line
                .split_once("=>")
                .map(|(name, replacement)| (name.trim(), replacement.trim()))
                .unwrap_or((line, ""));
            (name.to_string(), replacement.to_string())
        })
        .collect()
});

static COMMA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*,\s*").expect("valid regex"));
static PIPE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\|\s*").expect("valid regex"));
static PACKAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*([^<\s\[\(]+)").expect("valid regex"));

/// Package name of an alternative, without an architecture qualifier.
fn package_name(alternative: &str) -> &str {
    let name = PACKAGE
        .captures(alternative)
        .and_then(|captures| captures.get(1))
        .map_or("", |m| m.as_str());
    name.split(':').next().unwrap_or("")
}

fn replacement(package: &str) -> Option<&'static str> {
    OBSOLETE
        .iter()
        .find(|(name, _)| name == package)
        .map(|(_, replacement)| replacement.as_str())
}

impl SourceRule for TestsuiteDependsOnObsoletePackage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(sections) = tests_control_sections(files) else {
            return;
        };
        for message in obsolete_messages(&sections, false) {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

/// The `Depends` messages of `sections`. With `ored` false these are the
/// alternatives lintian reports as `testsuite-depends-on-obsolete-package`;
/// with `ored` true, the obsolete alternatives beside a current one, which
/// lintian reports as `testsuite-ored-depends-on-obsolete-package`.
pub(super) fn obsolete_messages(sections: &[Section], ored: bool) -> Vec<String> {
    let mut messages = Vec::new();
    for section in sections {
        if !section.declares("Depends") {
            continue;
        }
        let depends = section.unfolded_value("Depends");
        for dependency in COMMA.split(depends.trim()) {
            let alternatives: Vec<&str> = PIPE.split(dependency).collect();
            let obsolete: Vec<&str> = alternatives
                .iter()
                .copied()
                .filter(|alternative| replacement(package_name(alternative)).is_some())
                .collect();
            let first = alternatives.first().map_or("", |first| package_name(first));
            for alternative in &obsolete {
                let package = package_name(alternative);
                let beside_current = package != first && obsolete.len() != alternatives.len();
                if beside_current != ored {
                    continue;
                }
                let suffix = replacement(package)
                    .filter(|value| !value.is_empty())
                    .map(|value| format!(" => {value}"))
                    .unwrap_or_default();
                messages.push(format!("Depends: {alternative}{suffix}"));
            }
        }
    }
    messages
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn tree(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/tests/control", TestFile::new(body))]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no tests"))],
            tree("Test-Command: true\n"),
            tree("Test-Command: true\nDepends: @, libc6\n"),
            tree("Test-Command: true\nDepends: current | gaim\n"),
            tree("Test-Command: true\n# Depends: gaim\n"),
            tree("Test-Command: true\nDepends: new-package | xbase-clients\n"),
        ];
        let fail = vec![
            tree("Test-Command: true\nDepends: @, gaim (>= 0.1), pkg-config\n"),
            tree("Test-Command: true\nDepends: gaim | xbase-clients\n"),
            tree("Test-Command: true\nDepends:\n @,\n pkg-config:any\n"),
        ];
        SourceTreeTester::new(TestsuiteDependsOnObsoletePackage, pass, fail).test_and_snapshot();
    }
}
