use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::description_field::description;

pub struct PossibleUnindentedListInExtendedDescription;

declare_rule! {
    /// The extended description has a line that starts with a dash or an
    /// asterisk after a single space, which looks like a list whose items
    /// were not indented. Reported once, on the third such line of the
    /// description, as lintian does: one or two are not taken as a list.
    ///
    /// Reads the Installation control. Lines that are malformed control
    /// statements are not counted here. Extra is `line N`, N counting from 1
    /// at the first line after the synopsis. Location is empty. Udebs are
    /// checked too.
    PossibleUnindentedListInExtendedDescription,
    code = "LN1234",
    tag = "possible-unindented-list-in-extended-description",
    default_selected = true,
    experimental = false,
}

static INVALID_CONTROL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ \.\s*\S|^ \s+\.\s*$").expect("valid regex"));
static LIST_ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^ [\-*]").expect("valid regex"));

impl BinaryPackageRule for PossibleUnindentedListInExtendedDescription {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        let mut unindented = 0usize;
        for (position, line) in description.numbered_lines() {
            if INVALID_CONTROL.is_match(line) {
                continue;
            }
            if LIST_ITEM.is_match(line) {
                // Perl's `$unindented_list++ == 2`: the third occurrence.
                if unindented == 2 {
                    ctx.diagnostic(Diagnostic::warn(format!("line {position}")));
                }
                unindented += 1;
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
            // Indented items are fine.
            description("synopsis\n  - one\n  - two\n  - three"),
            // Two unindented items are not yet a list.
            description("synopsis\n - one\n - two"),
            // A dash after text is not at the start of a line.
            description("synopsis\n a - b\n a - c\n a - d"),
            // Only the synopsis, or only the dash in the synopsis.
            description("- synopsis\n A line."),
        ];
        let fail = vec![
            description("synopsis\n - one\n - two\n - three"),
            description("synopsis\n * one\n text\n * two\n * three"),
            // The third one is reported, not the fourth.
            description("synopsis\n - one\n - two\n - three\n - four"),
            // Control statements do not count.
            description("synopsis\n - one\n .text\n - two\n - three"),
            description("synopsis\n - one\n - two\n - three").udeb(),
        ];
        BinaryPackageTester::new(PossibleUnindentedListInExtendedDescription, pass, fail)
            .test_and_snapshot();
    }
}
