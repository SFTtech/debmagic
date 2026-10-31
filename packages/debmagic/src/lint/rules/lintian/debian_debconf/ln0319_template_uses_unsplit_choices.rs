use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::{files, templates};

pub struct TemplateUsesUnsplitChoices;

declare_rule! {
    /// A source template still uses the unsplit `_Choices` field.
    ///
    /// The Check reads `debian/templates` and `debian/<installable>.templates`
    /// from the packaging tree, not the templates installed in a binary
    /// package. The template must declare both `Template` and `_Choices`.
    /// `__Choices` and `Choices` do not count. The field name is
    /// case-insensitive. Extra is the `Template` value. The Location is
    /// `_Choices`. A file that is not valid UTF-8 debconf Deb822 is skipped.
    TemplateUsesUnsplitChoices,
    code = "LN0319",
    tag = "template-uses-unsplit-choices",
    default_selected = true,
    experimental = false,
}

impl SourceRule for TemplateUsesUnsplitChoices {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let paths = catalog_paths(ctx);
        let hits = {
            let Some(tree) = ctx.files() else {
                return;
            };
            let mut hits = Vec::new();
            for path in paths {
                let Some(text) = files::read_utf8(tree, &path) else {
                    continue;
                };
                let Some(parsed) = templates::parse(&text.text) else {
                    continue;
                };
                for template in &parsed {
                    if !template.declares("Template") || !template.declares("_Choices") {
                        continue;
                    }
                    hits.push((
                        text.path.clone(),
                        template.position("_Choices"),
                        template.value("Template"),
                    ));
                }
            }
            hits
        };
        for (path, line, name) in hits {
            ctx.diagnostic(Diagnostic::warn(name).with_location(files::located(path, line)));
        }
    }
}

fn catalog_paths(ctx: &SourceContext<'_>) -> Vec<String> {
    let mut paths = vec!["debian/templates".to_string()];
    if let Some(control) = ctx.debian_control() {
        for installable in control.installables() {
            paths.push(format!("debian/{}.templates", installable.name()));
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase, TestFile};

    use super::*;

    fn control() -> &'static str {
        "\
Source: example
Maintainer: Example <ex@example.com>

Package: debconf-test
Architecture: all
Description: example
 extra

Package: other-pkg
Architecture: all
Description: example
 extra
"
    }

    #[test]
    fn test() {
        let pass = vec![
            TestCase::from(Vec::<(&str, TestFile)>::new()),
            TestCase::from([(
                "debian/templates",
                "Template: debconf/split\nType: select\n__Choices: a, b\nDescription: Pick\n",
            )]),
            TestCase::from([(
                "debian/templates",
                "Template: debconf/plain\nType: select\nChoices: a, b\nDescription: Pick\n",
            )]),
            TestCase::from([(
                "_Choices is not under debian",
                "Template: a/b\n_Choices: a\n",
            )]),
            TestCase::from([(
                "debian/templates",
                "_Choices: a, b\nDescription: no template name\n",
            )]),
            TestCase::from([("debian/templates", "Template: a/b\n \n_Choices: x\n")]),
            TestCase::from([(
                "debian/templates",
                TestFile::from_bytes(b"Template: a/b\n_Choices: x\n\xff".to_vec()),
            )]),
            TestCase::from([("debian/templates", TestFile::symlink("missing"))]),
            TestCase::from([
                ("debian/control", control()),
                (
                    "debian/not-listed.templates",
                    "Template: debconf/hidden\n_Choices: a, b\n",
                ),
            ]),
            TestCase::from([(
                "debian/templates",
                "This is not deb822\nTemplate: a/b\n_Choices: x\n",
            )]),
        ];
        let fail = vec![
            TestCase::from([(
                "debian/templates",
                "\
Template: debconf/unsplit-choices
Type: select
_Choices: left, right, up, down
_Description: Pick the odd one out

Template: debconf/second
_choices: a, b
",
            )]),
            TestCase::from([
                ("debian/control", control()),
                (
                    "debian/templates",
                    "Template: debconf/unsplit-choices\n_Choices: left, right\n",
                ),
                (
                    "debian/debconf-test.templates",
                    "Template: debconf/testmulti-escape\n_Choices: foo\\, bar, boo\n",
                ),
                (
                    "debian/other-pkg.templates",
                    "Template: other/choice\n_Choices: a, b\n",
                ),
            ]),
            TestCase::from([
                ("debian/control", "This is not deb822\n"),
                (
                    "debian/templates",
                    "Template: debconf/still\n_Choices: a, b\n",
                ),
                (
                    "debian/debconf-test.templates",
                    "Template: debconf/ignored\n_Choices: a, b\n",
                ),
            ]),
            TestCase::from([("debian/templates", "Template:\n_Choices: a, b\n")]),
        ];
        SourceTreeTester::new(TemplateUsesUnsplitChoices, pass, fail).test_and_snapshot();
    }
}
