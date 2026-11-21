use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::description_field::description;

pub struct DescriptionIsDhMakeTemplate;

declare_rule! {
    /// The synopsis or the first extended line of the `Description` is the
    /// `dh_make` placeholder that was never replaced.
    ///
    /// Reads the Installation control. The synopsis (when not blank) contains
    /// `<insert up to 60 chars description>`, or else the first extended line
    /// (position 1, so not a ` .` separator) contains `<insert long
    /// description, indented with spaces>`. The synopsis placeholder is
    /// reported with empty extra, the extended line with `line 1`. The
    /// `debmake` `missing` placeholder shares the template flag with this
    /// Tag: when the synopsis is `missing`, lintian reports that Tag instead
    /// and the extended line is not checked here. Udebs are checked too.
    DescriptionIsDhMakeTemplate,
    code = "LN0923",
    tag = "description-is-dh_make-template",
    default_selected = true,
    experimental = false,
}

const DH_MAKE_SYNOPSIS: &str = "<insert up to 60 chars description>";
const DH_MAKE_EXTENDED: &str = "<insert long description, indented with spaces>";

impl BinaryPackageRule for DescriptionIsDhMakeTemplate {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        let mut template = false;
        if !description.synopsis.trim().is_empty() {
            if description.synopsis.trim() == "missing" {
                template = true;
            } else if description.synopsis.contains(DH_MAKE_SYNOPSIS) {
                ctx.diagnostic(Diagnostic::error(""));
                template = true;
            }
        }
        if template {
            return;
        }
        let first_is_template = matches!(
            description.numbered_lines().next(),
            Some((1, line)) if line.contains(DH_MAKE_EXTENDED)
        );
        if first_is_template {
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
            // Only position 1 counts: a separator before the placeholder moves it to 2.
            description("synopsis\n .\n <insert long description, indented with spaces>"),
            description("missing\n <insert long description, indented with spaces>"),
            description("synopsis\n Text.").udeb(),
        ];
        let fail = vec![
            description("<insert up to 60 chars description>\n Text."),
            description("<insert up to 60 chars description>\n Text.").udeb(),
            description(
                "<insert up to 60 chars description>\n <insert long description, indented with spaces>",
            ),
            description("synopsis\n <insert long description, indented with spaces>\n Text."),
            description("synopsis\n <insert long description, indented with spaces>").udeb(),
        ];
        BinaryPackageTester::new(DescriptionIsDhMakeTemplate, pass, fail).test_and_snapshot();
    }
}
