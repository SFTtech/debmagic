use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, templates};

pub struct TooLongShortDescriptionInTemplates;

declare_rule! {
    /// The short description is longer than 75 characters.
    ///
    /// Length is in characters. `for internal use` in that line skips the
    /// template. A udeb whose `Type` is `text` is skipped; other types on a
    /// udeb are not. Extra is the template name. The Location is
    /// `Description`.
    TooLongShortDescriptionInTemplates,
    code = "LN0321",
    tag = "too-long-short-description-in-templates",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for TooLongShortDescriptionInTemplates {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let udeb = ctx.is_udeb();
        let Some(loaded) = load(ctx) else {
            return;
        };
        let mut hits = Vec::new();
        for template in &loaded.templates {
            let short = templates::short_description(template);
            if templates::for_internal_use(&short) {
                continue;
            }
            if short.chars().count() <= templates::MAXIMUM_SHORT_LENGTH {
                continue;
            }
            if udeb && template.value("Type") == "text" {
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

    fn synopsis(name: &str, kind: &str, length: usize) -> String {
        format!(
            "Template: {name}\nType: {kind}\nDescription: {}\n",
            "a".repeat(length)
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            templates(&synopsis("debconf/ok", "boolean", 75)),
            templates(&format!(
                "Template: debconf/internal\nType: boolean\nDescription: for internal use {}\n",
                "a".repeat(76)
            )),
            templates(&synopsis("debconf/text", "text", 76)).udeb(),
            templates("Template: debconf/under\nType: boolean\n_Description: not the field\n"),
        ];
        let fail = vec![
            templates(&synopsis("debconf/too-long-short-desc", "boolean", 76)),
            templates(&format!(
                "Template: debconf/accent\nType: note\nDescription: {}\n",
                "é".repeat(76)
            )),
            templates(&synopsis("debconf/udeb-note", "note", 76)).udeb(),
            templates(&synopsis("debconf/text-deb", "text", 76)),
        ];
        BinaryPackageTester::new(TooLongShortDescriptionInTemplates, pass, fail)
            .test_and_snapshot();
    }
}
