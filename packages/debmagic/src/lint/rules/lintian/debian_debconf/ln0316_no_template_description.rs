use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, templates};

pub struct NoTemplateDescription;

declare_rule! {
    /// A debconf template has neither `Description` nor `_Description`.
    ///
    /// Both names are case-insensitive. A field that is present but blank
    /// counts as missing. Extra is the template name, or `no-template-name`.
    /// The Location is the first field of the paragraph. A templates file
    /// that is not valid debconf Deb822 is skipped.
    NoTemplateDescription,
    code = "LN0316",
    tag = "no-template-description",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for NoTemplateDescription {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(loaded) = load(ctx) else {
            return;
        };
        let mut hits = Vec::new();
        for template in &loaded.templates {
            if !template.value("Description").is_empty()
                || !template.value("_Description").is_empty()
            {
                continue;
            }
            hits.push((
                template.paragraph_line(),
                templates::template_name(template),
            ));
        }
        for (line, name) in hits {
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
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            templates("Template: debconf/ok\nType: string\nDescription: Name:\n"),
            templates("Template: debconf/under\nType: string\n_Description: Name:\n"),
            templates("Template: debconf/both\nDescription: Name:\n_Description:\n"),
            templates("type: string\n_description: Name:\n"),
            templates("Template: a/b\n \nDescription:\n"),
            templates("Template: a/b\nTemplate: again\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
                "templates",
                TestFile::from_bytes(b"Template: a/b\nType: string\n\xff"),
            ),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("config", TestFile::new("#!/bin/sh\n")),
        ];
        let fail = vec![
            templates("Template: debconf/no-description\nType: error\n"),
            templates("Template: debconf/blank\nType: string\nDescription:\n_Description:   \n"),
            templates("Type: string\n"),
            templates("# comment\nTemplate: debconf/later\nType: note\n"),
            templates("Template: debconf/udeb\nType: text\n").udeb(),
        ];
        BinaryPackageTester::new(NoTemplateDescription, pass, fail).test_and_snapshot();
    }
}
