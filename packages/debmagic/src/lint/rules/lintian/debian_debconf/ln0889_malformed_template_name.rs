use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, templates};

pub struct MalformedTemplateName;

declare_rule! {
    /// A template's `Template` name has no `X/Y` shape.
    ///
    /// The installed `templates` must parse as debconf Deb822, so a broken
    /// file reports nothing here. Each paragraph that declares `Template` is
    /// checked. The name is malformed unless it contains a character from
    /// `A-Za-z0-9.+-`, a `/`, and another such character, anywhere in the
    /// value. The Extra is the name and the Location is the `Template` field.
    /// Udebs are checked too.
    MalformedTemplateName,
    code = "LN0889",
    tag = "malformed-template-name",
    default_selected = true,
    experimental = false,
}

static WELL_FORMED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z0-9.+-]/[A-Za-z0-9.+-]").expect("valid regex"));

impl BinaryPackageRule for MalformedTemplateName {
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
                .filter(|template| template.declares("Template"))
                .filter_map(|template| {
                    let name = template.value("Template");
                    (!WELL_FORMED.is_match(&name)).then(|| (template.position("Template"), name))
                })
                .map(|(line, name)| (file.path.clone(), line, name))
                .collect::<Vec<_>>()
        };
        for (path, line, name) in hits {
            ctx.diagnostic(Diagnostic::error(name).with_location(files::located(path, line)));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn templates(name: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
            "templates",
            TestFile::new(format!(
                "Template: {name}\nType: string\nDescription: Pick\n"
            )),
        )
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            templates("debconf/x"),
            templates("pkg.name+x-y/sub"),
            templates("a/b/c"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
                "templates",
                TestFile::new("Type: string\nDescription: no name\n"),
            ),
        ];
        let fail = vec![
            templates("debconf-malformed-template"),
            templates("a/"),
            templates("/a"),
            templates("a / b"),
            templates("é/b"),
            templates("bad-name").udeb(),
        ];
        BinaryPackageTester::new(MalformedTemplateName, pass, fail).test_and_snapshot();
    }
}
