use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, templates};

pub struct UsingImperativeFormInTemplates;

declare_rule! {
    /// A select or multiselect short description starts with an imperative.
    ///
    /// The words are `Please`, `Choose` (`Chose`, with any run of `o`),
    /// `Enter`, `Select`, `Specify`, and `Give`, case-sensitive and only at
    /// the start of the short description. `for internal use` skips the
    /// template. Extra is the template name. The Location is `Description`.
    /// Udebs are not checked.
    UsingImperativeFormInTemplates,
    code = "LN0240",
    tag = "using-imperative-form-in-templates",
    default_selected = true,
    experimental = false,
}

static IMPERATIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(Please|Cho+se|Enter|Select|Specify|Give)").expect("valid regex")
});

impl BinaryPackageRule for UsingImperativeFormInTemplates {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(loaded) = load(ctx) else {
            return;
        };
        let mut hits = Vec::new();
        for template in &loaded.templates {
            let kind = template.value("Type");
            if kind != "select" && kind != "multiselect" {
                continue;
            }
            let short = templates::short_description(template);
            if templates::for_internal_use(&short) || !IMPERATIVE.is_match(&short) {
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

    fn select(description: &str) -> BinaryPackageCase {
        templates(&format!(
            "Template: debconf/choice\nType: select\nChoices: a, b\nDescription: {description}\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            select("The color"),
            select("for internal use Choose"),
            select("choose the color"),
            select("Now Please choose"),
            templates("Template: debconf/string\nType: string\nDescription: Select this:\n"),
            select("Select either x or y").udeb(),
        ];
        let fail = vec![
            select("Select either x or y"),
            select("Please pick"),
            select("Choose one"),
            select("Chose this"),
            select("Enter a value"),
            select("Specify the thing"),
            select("Give a name"),
            templates(
                "Template: debconf/multi\nType: multiselect\nChoices: a, b\nDescription: Please pick\n",
            ),
        ];
        BinaryPackageTester::new(UsingImperativeFormInTemplates, pass, fail).test_and_snapshot();
    }
}
