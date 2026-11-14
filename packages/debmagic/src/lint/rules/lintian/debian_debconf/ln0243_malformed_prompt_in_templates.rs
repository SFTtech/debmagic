use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, templates};

pub struct MalformedPromptInTemplates;

declare_rule! {
    /// A string or password short description is not an open prompt.
    ///
    /// It should end in `:` and not start with `what`, `who`, `when`,
    /// `where`, `which`, or `how`. Either failure is enough. Select templates
    /// are not checked. `for internal use` skips the template. Extra is the
    /// template name. The Location is `Description`. Udebs are not checked.
    MalformedPromptInTemplates,
    code = "LN0243",
    tag = "malformed-prompt-in-templates",
    default_selected = true,
    experimental = false,
}

static QUESTION_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(what|who|when|where|which|how)").expect("valid regex"));

impl BinaryPackageRule for MalformedPromptInTemplates {
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
            if kind != "string" && kind != "password" {
                continue;
            }
            let short = templates::short_description(template);
            if short.is_empty() || templates::for_internal_use(&short) {
                continue;
            }
            if !short.ends_with(':') || QUESTION_WORD.is_match(&short) {
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

    fn prompt(kind: &str, description: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
            "templates",
            TestFile::new(format!(
                "Template: debconf/teststring\nType: {kind}\nDescription: {description}\n"
            )),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            prompt("string", "IP address:"),
            prompt("string", "for internal use"),
            prompt("password", "Password:"),
            prompt("string", ""),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
                "templates",
                TestFile::new(
                    "Template: debconf/select\nType: select\nDescription: What is this?\n",
                ),
            ),
            prompt("string", "What is the name:").udeb(),
        ];
        let fail = vec![
            prompt("string", "What is the name of this package checker:"),
            prompt("string", "Hostname"),
            prompt("password", "who goes there:"),
            prompt("string", "how many:"),
            prompt("password", "Secret"),
        ];
        BinaryPackageTester::new(MalformedPromptInTemplates, pass, fail).test_and_snapshot();
    }
}
