use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::{DebianControl, InstallablePackageType};
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

pub struct DuplicateLongDescription;

declare_rule! {
    /// Two or more installables of `debian/control` (udebs not counted) have
    /// the same extended description: the lines of `Description` after the
    /// synopsis, trimmed at both ends. The extra is the sorted names of the
    /// packages, separated by spaces, and the Location is `debian/control`.
    ///
    /// A description that holds a `${...}` substitution variable is not
    /// compared, as lintian assumes substvars are handled. An installable
    /// without an extended description is not counted. Info severity and not
    /// selected by default.
    DuplicateLongDescription,
    code = "LN1519",
    tag = "duplicate-long-description",
    default_selected = false,
    experimental = false,
}

/// The extended description: every line of `Description` after the first.
pub(super) fn extended_part(description: &str) -> &str {
    description
        .split_once('\n')
        .map_or("", |(_, extended)| extended)
}

static SUBSTVAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$\{.+\}").expect("valid regex"));

impl SourceRule for DuplicateLongDescription {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        for names in duplicate_names(control, extended_part) {
            ctx.diagnostic(
                Diagnostic::warn(names.join(" "))
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path: PathBuf::from("debian/control"),
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

/// The sorted package names of each group of two or more installables whose
/// `part` of the Description (`extended_part`, or the synopsis) is the same,
/// in key order. udebs, empty parts and keys with a substvar are left out.
pub(super) fn duplicate_names(control: &DebianControl, part: fn(&str) -> &str) -> Vec<Vec<String>> {
    let mut by_text: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for installable in control.installables() {
        if installable.package_type() == InstallablePackageType::Udeb {
            continue;
        }
        let Some(description) = installable.fields().untrimmed_value("Description") else {
            continue;
        };
        let text = part(description).trim();
        if text.is_empty() || SUBSTVAR.is_match(text) {
            continue;
        }
        by_text
            .entry(text.to_string())
            .or_default()
            .push(installable.name().to_string());
    }
    by_text
        .into_values()
        .filter(|names| names.len() > 1)
        .map(|mut names| {
            names.sort();
            names
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase};

    use super::*;

    const HEAD: &str = "Source: example\nMaintainer: A <a@example.com>\n\n";

    fn control(paragraphs: &str) -> TestCase {
        TestCase::from([("debian/control", format!("{HEAD}{paragraphs}"))])
    }

    #[test]
    fn source() {
        let pass = vec![
            control(
                "Package: pkg-a\nArchitecture: all\nDescription: one\n long text a\n\nPackage: pkg-b\nArchitecture: all\nDescription: two\n long text b\n",
            ),
            // Same synopsis, different extended text.
            control(
                "Package: pkg-a\nArchitecture: all\nDescription: same\n long text a\n\nPackage: pkg-b\nArchitecture: all\nDescription: same\n long text b\n",
            ),
            // A udeb does not count.
            control(
                "Package: pkg-a\nArchitecture: all\nDescription: one\n long text\n\nPackage: pkg-udeb\nPackage-Type: udeb\nArchitecture: all\nDescription: two\n long text\n",
            ),
            // Substitution variables are not compared.
            control(
                "Package: pkg-a\nArchitecture: all\nDescription: ${x}\n ${y}\n\nPackage: pkg-b\nArchitecture: all\nDescription: ${x}\n ${y}\n",
            ),
            // An installable without an extended description.
            control(
                "Package: pkg-a\nArchitecture: all\nDescription: one\n\nPackage: pkg-b\nArchitecture: all\nDescription: two\n",
            ),
        ];
        let fail = vec![
            control(
                "Package: pkg-a\nArchitecture: all\nDescription: one\n long text\n\nPackage: pkg-b\nArchitecture: all\nDescription: two\n long text\n",
            ),
            // Trimmed at both ends, so the blank line before the text does not matter.
            control(
                "Package: pkg-a\nArchitecture: all\nDescription: one\n  shared\n\nPackage: pkg-b\nArchitecture: all\nDescription: two\n  shared\n\nPackage: pkg-c\nArchitecture: all\nDescription: three\n shared\n",
            ),
        ];
        SourceTreeTester::new(DuplicateLongDescription, pass, fail).test_and_snapshot();
    }
}
