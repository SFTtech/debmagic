use std::sync::LazyLock;

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

impl BinaryPackageRule for DescriptionMentionsPlannedFeatures {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        for (position, line) in description.numbered_lines() {
            if PLANNED_FEATURES.iter().any(|regex| regex.is_match(line)) {
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
