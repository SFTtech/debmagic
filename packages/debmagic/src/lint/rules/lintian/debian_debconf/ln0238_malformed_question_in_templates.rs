use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, templates};

pub struct MalformedQuestionInTemplates;

declare_rule! {
    /// The short description of a boolean template does not contain `?`.
    ///
    /// Only the first line counts; a question mark in the extended
    /// description does not. `for internal use` skips the template. An empty
    /// or missing `Description` is still a question with no mark, and then
    /// the Location has no line. Extra is the template name. Udebs are not
    /// checked.
    MalformedQuestionInTemplates,
    code = "LN0238",
    tag = "malformed-question-in-templates",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MalformedQuestionInTemplates {
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
            if templates::for_internal_use(&short) || short.contains('?') {
                continue;
            }
            hits.push((
                template.position("Description"),
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
            templates("Template: debconf/ok\nType: boolean\nDescription: Really?\n"),
            templates(
                "Template: debconf/internal\nType: boolean\nDescription: For internal use only\n",
            ),
            templates(
                "Template: debconf/string\nType: string\nDescription: Lintian is the best!\n",
            ),
            templates("Template: debconf/select\nType: select\nDescription: Pick one\n"),
            templates("Template: debconf/udeb\nType: boolean\nDescription: Decide\n").udeb(),
        ];
        let fail = vec![
            templates(
                "Template: debconf/testboolean\nType: boolean\nDescription: Enter yes or no:\n Do you want to answer this question?\n",
            ),
            templates(
                "Template: debconf/should-be-no-longer-a-problem\nType: boolean\nDescription: Decide, lintian\n",
            ),
            templates(
                "Template: debconf/malformed-question\nType: boolean\nDescription: Lintian is the best!\n",
            ),
            templates("Template: debconf/missing\nType: boolean\n"),
            templates("Template: debconf/underscore\nType: boolean\n_Description: Really?\n"),
        ];
        BinaryPackageTester::new(MalformedQuestionInTemplates, pass, fail).test_and_snapshot();
    }
}
