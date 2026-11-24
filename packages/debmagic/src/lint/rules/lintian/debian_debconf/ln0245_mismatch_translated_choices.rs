use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, templates};

pub struct MismatchTranslatedChoices;

declare_rule! {
    /// A `Choices-*` translation has a different number of choices.
    ///
    /// Items are split on a comma followed by whitespace. `\,` and `\ ` stay
    /// inside an item. The Location is the untranslated `Choices` field, not
    /// the translation. Extra is the template name and the translated field.
    /// Udebs are not checked.
    MismatchTranslatedChoices,
    code = "LN0245",
    tag = "mismatch-translated-choices",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MismatchTranslatedChoices {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(loaded) = load(ctx) else {
            return;
        };
        let mut hits = Vec::new();
        for template in &loaded.templates {
            if !template.declares("Choices") {
                continue;
            }
            let choices = template.value("Choices");
            if choices.chars().all(char::is_whitespace) {
                continue;
            }
            let count = templates::count_choices(&choices);
            let line = template.position("Choices");
            let name = templates::template_name(template);
            for field in template.fields() {
                if !field.name.starts_with("Choices-") {
                    continue;
                }
                let translated = template.value(&field.name);
                if templates::count_choices(&translated) != count {
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

fn load(ctx: &mut BinaryPackageContext<'_>) -> Option<templates::TemplatesFile> {
    let files = ctx.control_files()?;
    if !files::gate(files).open() {
        return None;
    }
    templates::load_templates(files)
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
            templates(
                "\
Template: debconf/ok
Type: select
Choices: sakana, sake
Choices-de: fisch, reis
Description: Preference
",
            ),
            templates(
                "\
Template: debconf/escape
Type: multiselect
Choices: foo\\, bar, boo
Choices-de: a, b
Description: Escaped
",
            ),
            templates("Template: debconf/none\nType: select\nDescription: Missing\n"),
            templates("Template: debconf/blank\nType: select\nChoices:   \nChoices-de: a\n"),
            templates("Template: debconf/tight\nType: select\nChoices: x,y\nChoices-de: eins\n"),
            templates(
                "\
Template: debconf/bad
Type: select
Choices: a, b
Choices-de: nur-eins
",
            )
            .udeb(),
        ];
        let fail = vec![
            templates(
                "\
Template: debconf/select-empty-trans-choices
Type: select
Choices: sakana, sake
Choices-Da:
Description: Preference
",
            ),
            templates(
                "\
Template: debconf/testmulti-escape
Type: multiselect
Choices: foo\\, bar, boo
Choices-de.UTF-8: foo, bar, boo
Description: test escaped comma
",
            ),
            templates(
                "\
Template: debconf/mixed
Type: select
Choices: a, b
Choices-de: eins, zwei
Choices-fr: seul
Description: Mixed
",
            ),
        ];
        BinaryPackageTester::new(MismatchTranslatedChoices, pass, fail).test_and_snapshot();
    }
}
