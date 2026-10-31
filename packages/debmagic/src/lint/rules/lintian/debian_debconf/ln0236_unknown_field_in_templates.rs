use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, templates};

pub struct UnknownFieldInTemplates;

declare_rule! {
    /// A debconf template field is not one of the known names.
    ///
    /// Known names are `Template`, `Type`, `Choices`, `Indices`, `Default`,
    /// `Description`, and `Help`. A language suffix (`Choices-de.UTF-8`) and
    /// one leading `_` are ignored; the remainder is case-sensitive, so
    /// `description` is unknown. Extra is the template name and the field.
    /// The Location is that field. Udebs are checked. A syntax error drops
    /// the file.
    UnknownFieldInTemplates,
    code = "LN0236",
    tag = "unknown-field-in-templates",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UnknownFieldInTemplates {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let hits = {
            let Some(files) = ctx.control_files() else {
                return;
            };
            if !files::gate(files).open() {
                return;
            }
            let Some(loaded) = templates::load_templates(files) else {
                return;
            };
            let mut hits = Vec::new();
            for template in &loaded.templates {
                let name = templates::template_name(template);
                for field in template.fields() {
                    if templates::known_field(&field.name) {
                        continue;
                    }
                    hits.push((
                        loaded.path.clone(),
                        field.line,
                        format!("{name} {}", field.name),
                    ));
                }
            }
            hits
        };
        for (path, line, message) in hits {
            ctx.diagnostic(
                Diagnostic::error(message).with_location(files::located(path, Some(line))),
            );
        }
    }
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
            templates(
                "\
Template: example/ok
Type: string
Choices: a, b
Choices-de.UTF-8: x, y
Indices: 1, 2
Default: a
_Choices: left, right
Description: Name:
 Help is known.
Help: more
",
            ),
            templates("Template: a/b\nType: string\nTemplate: again\nWeird: 1\n"),
            templates("Template: a/b\n \nWeird: 1\n"),
            templates("Template: a/b\nDescription whoops\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
                "templates",
                TestFile::from_bytes(b"Template: a/b\nWeird: 1\n\xff"),
            ),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("templates", TestFile::symlink("missing")),
            templates("Template: example/ok\nType: text\nDescription: Title\n").udeb(),
        ];
        let fail = vec![
            templates(
                "\
Template: debconf/unknown-field
Type: boolean
Description: Is the field below unknown?
Unknown-field: yes, it is.
",
            ),
            templates(
                "\
Template: debian-installer/debconf-udeb/title
Type: text
description: This is just a test
description-de.UTF-8: Dies ist nur ein Test
",
            )
            .udeb(),
            templates("# comment\nWeird: 1\n"),
            templates("Template: a/b\nType: string\nFoo: 1\nBar-baz: 2\n"),
        ];
        BinaryPackageTester::new(UnknownFieldInTemplates, pass, fail).test_and_snapshot();
    }
}
