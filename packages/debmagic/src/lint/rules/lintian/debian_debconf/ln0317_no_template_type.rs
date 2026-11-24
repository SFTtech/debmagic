use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, templates};

pub struct NoTemplateType;

declare_rule! {
    /// A debconf template does not declare `Type`.
    ///
    /// The field name is case-insensitive. A blank `Type` still counts as
    /// declared. Extra is the template name, or `no-template-name`. The
    /// Location is the first field of the paragraph. A templates file that
    /// is not valid debconf Deb822 is skipped. Udebs are checked.
    NoTemplateType,
    code = "LN0317",
    tag = "no-template-type",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for NoTemplateType {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(loaded) = load(ctx) else {
            return;
        };
        let mut hits = Vec::new();
        for template in &loaded.templates {
            if template.declares("Type") {
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
            templates("Template: debconf/lower\ntype: note\nDescription: Name:\n"),
            templates("Template: debconf/blank\nType:\nDescription: Name:\n"),
            templates("Template: a/b\n \nType: string\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("templates", TestFile::symlink("missing")),
        ];
        let fail = vec![
            templates("Template: debconf/no-type\nDescription: Missing type\n"),
            templates("Description: no name and no type\n"),
            templates("# comment\nTemplate: debconf/later\nDescription: Name:\n"),
            templates("Template: debconf/udeb\nDescription: Title\n").udeb(),
        ];
        BinaryPackageTester::new(NoTemplateType, pass, fail).test_and_snapshot();
    }
}
