use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, templates};

pub struct MakingAssumptionsAboutInterfacesInTemplates;

declare_rule! {
    /// A boolean template mentions the word `yes` as a widget label.
    ///
    /// The word is matched in the whole description, case-insensitive, when
    /// it is surrounded by a space, quote, or (after the word) a comma,
    /// semicolon, or period. `for internal use` skips the template. Extra is
    /// the template name. The Location is `Description`. Udebs are not
    /// checked.
    MakingAssumptionsAboutInterfacesInTemplates,
    code = "LN0242",
    tag = "making-assumptions-about-interfaces-in-templates",
    default_selected = true,
    experimental = false,
}

static YES_WIDGET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)[ '"]yes[ '",;.]"#).expect("valid regex"));

impl BinaryPackageRule for MakingAssumptionsAboutInterfacesInTemplates {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(loaded) = load(ctx) else {
            return;
        };
        let mut hits = Vec::new();
        for template in &loaded.templates {
            if template.value("Type") != "boolean" {
                continue;
            }
            let short = templates::short_description(template);
            if templates::for_internal_use(&short) {
                continue;
            }
            if YES_WIDGET.is_match(&template.value("Description")) {
                hits.push((
                    template.position("Description"),
                    templates::template_name(template),
                ));
            }
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
    if !files::gate(files).open() {
        return None;
    }
    templates::load_templates(files)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn boolean(description: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
            "templates",
            TestFile::new(format!(
                "Template: debconf/testboolean\nType: boolean\nDescription: {description}\n"
            )),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            boolean("Do you agree?"),
            boolean("Is yesterday fine?"),
            boolean("Is \"no longer\" a problem?"),
            boolean("yes or no?"),
            boolean("For internal use say \"yes\"."),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
                "templates",
                TestFile::new(
                    "Template: debconf/select\nType: select\nDescription: Pick \"yes\".\n",
                ),
            ),
            boolean("Enter yes or no:").udeb(),
        ];
        let fail = vec![
            boolean("Do you like Lintian?\n Pick \"yes\" if you do."),
            boolean("Enter yes or no:"),
            boolean("Answer 'Yes', please?"),
            boolean("Do you agree?\n Say yes."),
        ];
        BinaryPackageTester::new(MakingAssumptionsAboutInterfacesInTemplates, pass, fail)
            .test_and_snapshot();
    }
}
