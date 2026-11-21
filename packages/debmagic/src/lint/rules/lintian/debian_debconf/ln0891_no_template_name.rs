use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, templates};

pub struct NoTemplateName;

declare_rule! {
    /// A template paragraph has no `Template` field.
    ///
    /// The installed `templates` must parse as debconf Deb822; a broken file
    /// reports nothing here. The Location is the paragraph's first line, the
    /// line of its first field, and the Extra is empty. Udebs are checked too.
    NoTemplateName,
    code = "LN0891",
    tag = "no-template-name",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for NoTemplateName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let hits = {
            let Some(tree) = ctx.control_files() else {
                return;
            };
            let Some(file) = templates::load_templates(tree) else {
                return;
            };
            file.templates
                .iter()
                .filter(|template| !template.declares("Template"))
                .map(|template| (file.path.clone(), template.paragraph_line()))
                .collect::<Vec<_>>()
        };
        for (path, line) in hits {
            ctx.diagnostic(Diagnostic::error("").with_location(files::located(path, line)));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn templates(text: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("templates", TestFile::new(text))
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            templates("Template: debconf/x\nType: string\n"),
            templates("Template: debconf/x\nType: string\n\nTemplate: debconf/y\nType: note\n"),
            templates("Template: debconf/y\nType: note\n").udeb(),
        ];
        let fail = vec![
            templates("Type: string\nDescription: no name\n"),
            templates("# comment\nType: string\n"),
            templates("Template: debconf/x\nType: note\n\nType: string\n"),
            templates("Type: string\n").udeb(),
        ];
        BinaryPackageTester::new(NoTemplateName, pass, fail).test_and_snapshot();
    }
}
