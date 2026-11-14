use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, templates};

pub struct EmptyTranslatedChoices;

declare_rule! {
    /// A `Choices-*` translation is empty or blank.
    ///
    /// Only templates whose `Choices` is declared and not blank are checked.
    /// Extra is the template name and the translated field. The Location is the
    /// untranslated `Choices` field. Udebs are checked.
    EmptyTranslatedChoices,
    code = "LN0690",
    tag = "empty-translated-choices",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for EmptyTranslatedChoices {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        let Some(loaded) = templates::load_gated(files) else {
            return;
        };
        let mut hits = Vec::new();
        for template in &loaded.templates {
            if !template.declares("Choices") || template.value("Choices").is_empty() {
                continue;
            }
            let line = template.position("Choices");
            let name = templates::template_name(template);
            for field in template.fields() {
                if field.name.starts_with("Choices-") && template.value(&field.name).is_empty() {
                    hits.push((line, format!("{name} {}", field.name)));
                }
            }
        }
        for (line, message) in hits {
            ctx.diagnostic(
                Diagnostic::error(message).with_location(files::located(loaded.path.clone(), line)),
            );
        }
    }
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
            templates("Template: debconf/ok\nType: select\nChoices: a, b\nChoices-de: c, d\n"),
            templates("Template: debconf/no-source\nType: select\nChoices-de:\n"),
            templates("Template: debconf/blank-source\nType: select\nChoices:  \nChoices-de:\n"),
            templates("Template: debconf/other\nType: select\nChoices: a\nDescription-de:\n"),
        ];
        let fail = vec![
            templates(
                "Template: debconf/select-empty-trans-choices\nType: select\nChoices: sakana, sake\nChoices-Da:\nDescription: Preference\n",
            ),
            templates("Template: debconf/blank\nType: select\nChoices: a, b\nChoices-de:   \n"),
            templates(
                "Template: debconf/two\nType: select\nChoices: a, b\nChoices-de:\nChoices-fr: x, y\nChoices-it:\n",
            ),
            templates("Type: select\nChoices: a\nChoices-de:\n").udeb(),
        ];
        BinaryPackageTester::new(EmptyTranslatedChoices, pass, fail).test_and_snapshot();
    }
}
