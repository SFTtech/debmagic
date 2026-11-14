use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, templates};

pub struct MalformedTitleInTemplates;

declare_rule! {
    /// A note's short description ends with `.`, `?`, `;`, or `:`.
    ///
    /// `for internal use` skips the template. Extra is the template name.
    /// The Location is `Description`. Udebs are not checked.
    MalformedTitleInTemplates,
    code = "LN0244",
    tag = "malformed-title-in-templates",
    default_selected = true,
    experimental = false,
}

static TITLE_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[.?;:]$").expect("valid regex"));

impl BinaryPackageRule for MalformedTitleInTemplates {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(loaded) = load(ctx) else {
            return;
        };
        let mut hits = Vec::new();
        for template in &loaded.templates {
            if template.value("Type") != "note" {
                continue;
            }
            let short = templates::short_description(template);
            if templates::for_internal_use(&short) || !TITLE_END.is_match(&short) {
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

    fn note(description: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
            "templates",
            TestFile::new(format!(
                "Template: debconf/testnote\nType: note\nDescription: {description}\n"
            )),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            note("Important information"),
            note("for internal use."),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
                "templates",
                TestFile::new("Template: debconf/error\nType: error\nDescription: Boom.\n"),
            ),
            note("Aren't you tired?").udeb(),
        ];
        let fail = vec![
            note("Aren't you tired of all these debconf prompts?"),
            note("This should be a title and not a sentence."),
            note("Choose one:"),
            note("Stop;"),
        ];
        BinaryPackageTester::new(MalformedTitleInTemplates, pass, fail).test_and_snapshot();
    }
}
