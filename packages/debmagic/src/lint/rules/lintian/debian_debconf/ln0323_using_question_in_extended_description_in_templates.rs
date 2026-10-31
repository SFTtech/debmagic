use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, templates};

pub struct UsingQuestionInExtendedDescriptionInTemplates;

declare_rule! {
    /// The extended description contains a question.
    ///
    /// A `?` counts when the character before it is not `?` and the character
    /// after it is whitespace or the end of the extended description. A
    /// question in the short description does not count. `for internal use`
    /// in the short description skips the template. Extra is the template
    /// name. The Location is `Description`. Udebs are checked.
    UsingQuestionInExtendedDescriptionInTemplates,
    code = "LN0323",
    tag = "using-question-in-extended-description-in-templates",
    default_selected = true,
    experimental = false,
}

static QUESTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^\?]\?(?:\s+|$)").expect("valid regex"));

impl BinaryPackageRule for UsingQuestionInExtendedDescriptionInTemplates {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(loaded) = load(ctx) else {
            return;
        };
        let mut hits = Vec::new();
        for template in &loaded.templates {
            let short = templates::short_description(template);
            if templates::for_internal_use(&short) {
                continue;
            }
            let Some(extended) = templates::extended_description(template) else {
                continue;
            };
            if !QUESTION.is_match(&extended) {
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

    fn described(name: &str, description: &str) -> BinaryPackageCase {
        templates(&format!(
            "Template: {name}\nType: boolean\nDescription: {description}\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            described("debconf/short-only", "Really?"),
            described(
                "debconf/statement",
                "Do you like sakana?\n Or do you prefer sake.",
            ),
            described("debconf/glued", "Title\n foo?bar"),
            described("debconf/double", "Title\n ??"),
            described("debconf/internal", "for internal use\n Or do you?"),
        ];
        let fail = vec![
            described(
                "debconf/question-in-extended",
                "Do you like sakana?\n Or do you prefer sake?",
            ),
            described("debconf/end", "Title\n What?"),
            described("debconf/space", "Title\n a? more"),
            described("debconf/bare", "Title\n ?"),
            described("debconf/udeb", "Title\n What?").udeb(),
        ];
        BinaryPackageTester::new(UsingQuestionInExtendedDescriptionInTemplates, pass, fail)
            .test_and_snapshot();
    }
}
