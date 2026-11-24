use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, templates};

pub struct SelectWithoutChoices;

declare_rule! {
    /// A `select` or `multiselect` debconf template has no `Choices` field.
    ///
    /// The `templates` file must load as debconf Deb822, and `Type` must be
    /// exactly `select` or `multiselect` after trimming. The field name is
    /// case-insensitive. `_Choices` does not count as `Choices`. Extra is
    /// the template name, or `no-template-name`. The Location is the first
    /// line of the paragraph. Udebs are checked.
    SelectWithoutChoices,
    code = "LN1024",
    tag = "select-without-choices",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SelectWithoutChoices {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(loaded) = load(ctx) else {
            return;
        };
        let mut hits = Vec::new();
        for template in &loaded.templates {
            let is_select = template.declares("Type")
                && matches!(template.value("Type").as_str(), "select" | "multiselect");
            if is_select && !template.declares("Choices") {
                hits.push((
                    templates::template_name(template),
                    template.paragraph_line(),
                ));
            }
        }
        for (name, line) in hits {
            ctx.diagnostic(
                Diagnostic::error(name).with_location(files::located(loaded.path.clone(), line)),
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
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            templates("Template: debconf/ok\nType: select\nChoices: a, b\nDescription: Pick\n"),
            templates("Template: debconf/multi\nType: multiselect\nChoices: a, b\n"),
            templates("Template: debconf/string\nType: string\nDescription: Name\n"),
            templates("Template: debconf/note\nType: note\nDescription: Read this\n"),
            // The Type value is case-sensitive and must be a valid select.
            templates("Template: debconf/case\nType: Select\nDescription: Name\n"),
            // No Type at all: a different Tag reports it.
            templates("Template: debconf/no-type\nDescription: Name\n"),
            // A blank Choices still counts as declared.
            templates("Template: debconf/blank\nType: select\nChoices:\nDescription: x\n"),
            // A file that does not parse is dropped whole.
            templates("Template: a/b\n \nType: select\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("templates", TestFile::symlink("missing")),
        ];
        let fail = vec![
            templates("Template: debconf/no-choices\nType: select\nDescription: Missing\n"),
            templates("Template: debconf/multi\nType: multiselect\nDescription: Missing\n"),
            // `_Choices` is the unsplit form and does not declare `Choices`.
            templates("Template: debconf/unsplit\nType: select\n_Choices: a, b\n"),
            // Without a Template field the name is a placeholder.
            templates("Type: select\nDescription: no name\n"),
            templates("# comment\nTemplate: debconf/later\nType: select\n"),
            templates("Template: debconf/udeb\nType: select\n").udeb(),
        ];
        BinaryPackageTester::new(SelectWithoutChoices, pass, fail).test_and_snapshot();
    }
}
