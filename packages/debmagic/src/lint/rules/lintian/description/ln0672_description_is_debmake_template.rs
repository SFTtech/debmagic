use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::description_field::description;

pub struct DescriptionIsDebmakeTemplate;

declare_rule! {
    /// The synopsis or the first extended line of the `Description` is the
    /// `debmake` placeholder `missing`.
    ///
    /// Reads the Installation control. The synopsis (when not blank) or the
    /// first extended line (position 1, so not a ` .` separator) is the word
    /// `missing` in any letter case, with optional surrounding whitespace. One
    /// template Tag per description: lintian shares the flag with
    /// `description-is-dh_make-template`, so a `dh_make` placeholder in the
    /// synopsis suppresses the extended-line report and a report for the
    /// synopsis suppresses the extended line. Extra is empty for the synopsis
    /// and `line 1` for the extended line. Udebs are checked too.
    DescriptionIsDebmakeTemplate,
    code = "LN0672",
    tag = "description-is-debmake-template",
    default_selected = true,
    experimental = false,
}

static MISSING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*missing\s*$").expect("valid regex"));
static MISSING_SYNOPSIS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^missing\s*$").expect("valid regex"));

const DH_MAKE_SYNOPSIS: &str = "<insert up to 60 chars description>";

impl BinaryPackageRule for DescriptionIsDebmakeTemplate {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        let mut template = false;
        if !description.synopsis.trim().is_empty() {
            if MISSING_SYNOPSIS.is_match(&description.synopsis) {
                ctx.diagnostic(Diagnostic::error(""));
                template = true;
            } else if description.synopsis.contains(DH_MAKE_SYNOPSIS) {
                template = true;
            }
        }
        if template {
            return;
        }
        let first_is_missing = matches!(
            description.numbered_lines().next(),
            Some((1, line)) if MISSING.is_match(line)
        );
        if first_is_missing {
            ctx.diagnostic(Diagnostic::error("line 1"));
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
            description("synopsis\n A real description."),
            description("missing in action\n Text."),
            // Only the first extended line counts.
            description("synopsis\n Text.\n missing"),
            description("synopsis\n .\n missing"),
            // A dh_make placeholder in the synopsis shares the template flag.
            description("<insert up to 60 chars description>\n missing"),
            description("<insert up to 60 chars description>\n Text.").udeb(),
        ];
        let fail = vec![
            description("missing\n Text."),
            description("MISSING  \n Text."),
            description("synopsis\n missing"),
            description("synopsis\n  Missing  ").udeb(),
            // The synopsis report consumes the template flag.
            description("missing\n missing"),
        ];
        BinaryPackageTester::new(DescriptionIsDebmakeTemplate, pass, fail).test_and_snapshot();
    }
}
