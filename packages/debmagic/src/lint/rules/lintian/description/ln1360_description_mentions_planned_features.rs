use std::sync::LazyLock;

use regex::bytes::{Regex as BytesRegex, RegexBuilder as BytesRegexBuilder};
use regex::{Regex, RegexBuilder};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::data;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::description_field::description;

pub struct DescriptionMentionsPlannedFeatures;

declare_rule! {
    /// An extended description line announces a feature that is not there yet,
    /// such as `is planned` or `will soon support`.
    ///
    /// Each extended line (not the synopsis, and not a ` .` separator) is
    /// matched case-insensitively, unanchored, against the vendored
    /// `description/planned-features` regular expressions. A line matching
    /// one or more of them is reported once, with extra `(line N)` counting
    /// from 1 at the first extended line. Location is empty. Udebs are checked
    /// too.
    DescriptionMentionsPlannedFeatures,
    code = "LN1360",
    tag = "description-mentions-planned-features",
    default_selected = false,
    experimental = false,
}

/// The table in Unicode mode, for a line that is not ASCII.
static PLANNED_FEATURES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    data::lines(include_str!("../../../data/description/planned-features"))
        .filter_map(|pattern| {
            RegexBuilder::new(pattern)
                .case_insensitive(true)
                .build()
                .ok()
        })
        .collect()
});

/// The table in ASCII mode, for an ASCII line. Case folding and the classes
/// agree with Unicode mode on ASCII text, and ASCII compiles much faster.
static ASCII_PLANNED_FEATURES: LazyLock<Vec<BytesRegex>> = LazyLock::new(|| {
    data::lines(include_str!("../../../data/description/planned-features"))
        .filter_map(|pattern| {
            BytesRegexBuilder::new(pattern)
                .case_insensitive(true)
                .unicode(false)
                .build()
                .ok()
        })
        .collect()
});

/// Whether any table pattern matches `line`.
fn planned_feature(line: &str) -> bool {
    if line.is_ascii() {
        ASCII_PLANNED_FEATURES
            .iter()
            .any(|regex| regex.is_match(line.as_bytes()))
    } else {
        PLANNED_FEATURES.iter().any(|regex| regex.is_match(line))
    }
}

impl BinaryPackageRule for DescriptionMentionsPlannedFeatures {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        for (position, line) in description.numbered_lines() {
            if planned_feature(line) {
                ctx.diagnostic(
                    Diagnostic::warn(format!("(line {position})")).with_severity(Severity::Info),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    /// The ASCII table keeps every pattern of the Unicode table, so that the
    /// two agree on the same lines.
    #[test]
    fn ascii_table_has_every_pattern() {
        assert_eq!(
            super::ASCII_PLANNED_FEATURES.len(),
            super::PLANNED_FEATURES.len()
        );
    }

    use super::*;

    fn description(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Description", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            description("synopsis\n Tools for the job."),
            // The synopsis is not an extended line.
            description("This is planned\n Tools for the job."),
        ];
        let fail = vec![
            description("synopsis\n Support is planned for later."),
            description("synopsis\n First line.\n .\n Will Soon Support XML."),
            description("synopsis\n Tools.\n Another is also planned.").udeb(),
        ];
        BinaryPackageTester::new(DescriptionMentionsPlannedFeatures, pass, fail)
            .test_and_snapshot();
    }
}
