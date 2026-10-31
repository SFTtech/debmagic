use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, templates};

pub struct UsingFirstPersonInTemplates;

declare_rule! {
    /// The description speaks in the first person.
    ///
    /// The words are `I` (capital only), `My`/`my`, `We`/`we`, `Our`/`our`,
    /// `Ours`/`ours`, `mine`, `myself`, `ourself`, `me`, and `us`, bounded by
    /// whitespace or the ends of the whole description. `for internal use` in
    /// the short description skips the template. Extra is the template name.
    /// The Location is `Description`. Udebs are checked.
    UsingFirstPersonInTemplates,
    code = "LN0322",
    tag = "using-first-person-in-templates",
    default_selected = true,
    experimental = false,
}

static FIRST_PERSON: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:\A|\s)(?:I|[Mm]y|[Ww]e|[Oo]ur|[Oo]urs|mine|myself|ourself|me|us)(?:\z|\s)")
        .expect("valid regex")
});

impl BinaryPackageRule for UsingFirstPersonInTemplates {
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
            if !FIRST_PERSON.is_match(&template.value("Description")) {
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
            "Template: {name}\nType: select\nChoices: a, b\nDescription: {description}\n"
        ))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            described("debconf/ok", "The color"),
            described("debconf/lower-i", "i think this is fine"),
            described("debconf/contraction", "I'll do it"),
            described("debconf/user", "The user chooses"),
            described("debconf/team", "The team decides"),
            described("debconf/period", "mine."),
            described("debconf/internal", "for internal use\n We are not shown"),
            described("debconf/ours-longer", "ourselves"),
        ];
        let fail = vec![
            described("debconf/first-person", "I think Lintian is the best"),
            described("debconf/we", "We recommend this"),
            described("debconf/we-lower", "we recommend this"),
            described("debconf/our", "Our choice"),
            described("debconf/ours", "ours is better"),
            described("debconf/mine", "This is mine"),
            described("debconf/myself", "Ask myself"),
            described("debconf/ourself", "Ask ourself"),
            described("debconf/me", "Tell me"),
            described("debconf/us", "Tell us"),
            described("debconf/my", "My package"),
            described("debconf/extended", "The title\n I am in the extended part"),
            described("debconf/udeb", "I am a udeb").udeb(),
        ];
        BinaryPackageTester::new(UsingFirstPersonInTemplates, pass, fail).test_and_snapshot();
    }
}
