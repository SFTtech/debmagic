use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, templates};

pub struct SelectWithBooleanChoices;

declare_rule! {
    /// `Choices` is only `yes` and `no`, in either order.
    ///
    /// The Check does not require `Type` to be `select`. The match is
    /// case-insensitive, and space around the comma is optional. `_Choices`
    /// does not count. A blank `Choices` is skipped. Extra is the template
    /// name. The Location is `Choices`. Udebs are checked.
    SelectWithBooleanChoices,
    code = "LN0318",
    tag = "select-with-boolean-choices",
    default_selected = true,
    experimental = false,
}

static BOOLEAN_CHOICES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?:yes\s*,\s*no|no\s*,\s*yes)\s*$").expect("valid regex")
});

impl BinaryPackageRule for SelectWithBooleanChoices {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(loaded) = load(ctx) else {
            return;
        };
        let mut hits = Vec::new();
        for template in &loaded.templates {
            if !template.declares("Choices") {
                continue;
            }
            let choices = template.value("Choices");
            if choices.is_empty() || !BOOLEAN_CHOICES.is_match(&choices) {
                continue;
            }
            hits.push((
                template.position("Choices"),
                templates::template_name(template),
            ));
        }
        for (line, name) in hits {
            ctx.diagnostic(
                Diagnostic::warn(name).with_location(files::located(loaded.path.clone(), line)),
            );
        }
    }
}

fn load(ctx: &mut BinaryPackageContext<'_>) -> Option<templates::TemplatesFile> {
    let files = ctx.control_files()?;
    templates::load_gated(files)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn templates(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("templates", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            templates("Template: debconf/ok\nType: select\nChoices: a, b\nDescription: Pick\n"),
            templates("Template: debconf/three\nType: select\nChoices: yes, no, maybe\n"),
            templates("Template: debconf/under\nType: select\n_Choices: yes, no\n"),
            templates("Template: debconf/blank\nType: select\nChoices:\n"),
            templates("Template: debconf/spaces\nType: select\nChoices:   \n"),
            templates("Template: debconf/yeah\nType: note\nChoices: yeah, no\n"),
        ];
        let fail = vec![
            templates("Template: debconf/select-boolean-choice\nType: select\nChoices: yes, no\n"),
            templates("Template: debconf/reverse\nType: select\nChoices: no, yes\n"),
            templates("Template: debconf/tight\nType: string\nChoices: yes,no\n"),
            templates("Template: debconf/case\nType: note\nChoices: Yes , No\n"),
            templates("choices: YES, NO\n").udeb(),
        ];
        BinaryPackageTester::new(SelectWithBooleanChoices, pass, fail).test_and_snapshot();
    }
}
