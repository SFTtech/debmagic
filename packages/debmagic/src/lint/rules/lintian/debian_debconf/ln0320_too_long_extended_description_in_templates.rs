use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, templates};

pub struct TooLongExtendedDescriptionInTemplates;

declare_rule! {
    /// The extended description wraps to more than 20 lines.
    ///
    /// Wrapping breaks a line longer than 80 characters at the last space at
    /// or before that column, otherwise at the first space. `for internal use`
    /// in the short description skips the template. Extra is the template
    /// name. The Location is `Description`. Udebs are checked.
    TooLongExtendedDescriptionInTemplates,
    code = "LN0320",
    tag = "too-long-extended-description-in-templates",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for TooLongExtendedDescriptionInTemplates {
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
            if extended.is_empty()
                || templates::wrapped_line_count(&extended) <= templates::MAXIMUM_EXTENDED_LINES
            {
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

    fn extended(lines: usize) -> String {
        let body = [" more"].repeat(lines).join("\n");
        format!("Template: debconf/long\nType: string\nDescription: Name:\n{body}\n")
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            templates(&extended(20)),
            templates("Template: debconf/short\nType: string\nDescription: Name:\n One line.\n"),
            templates(&format!(
                "Template: debconf/internal\nType: string\nDescription: for internal use\n{}\n",
                [" more"].repeat(21).join("\n")
            )),
            templates(&format!(
                "Template: debconf/word\nType: string\nDescription: Name:\n {}\n",
                "a".repeat(200)
            )),
        ];
        let wide = format!("{} {}", "a".repeat(40), "b".repeat(50));
        let mut wrapped = String::from("Template: debconf/wrap\nType: note\nDescription: Name:\n");
        for _ in 0..11 {
            wrapped.push(' ');
            wrapped.push_str(&wide);
            wrapped.push('\n');
        }
        let fail = vec![
            templates(&extended(21)),
            templates(&wrapped),
            templates(&extended(21)).udeb(),
        ];
        BinaryPackageTester::new(TooLongExtendedDescriptionInTemplates, pass, fail)
            .test_and_snapshot();
    }
}
