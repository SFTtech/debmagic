use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, templates};

pub struct BooleanTemplateHasBogusDefault;

declare_rule! {
    /// A `boolean` template's `Default` is neither `true` nor `false`.
    ///
    /// `Type` must be exactly `boolean` and `Default` declared (a blank value
    /// is bogus too). `_Default` does not count. Extra is the template name and
    /// the default. The Location is `Default`. Udebs are checked.
    BooleanTemplateHasBogusDefault,
    code = "LN0689",
    tag = "boolean-template-has-bogus-default",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BooleanTemplateHasBogusDefault {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        let Some(loaded) = templates::load_gated(files) else {
            return;
        };
        let mut hits = Vec::new();
        for template in &loaded.templates {
            if !template.declares("Type") || template.value("Type") != "boolean" {
                continue;
            }
            if !template.declares("Default") {
                continue;
            }
            let default = template.value("Default");
            if default == "true" || default == "false" {
                continue;
            }
            hits.push((
                template.position("Default"),
                format!("{} {default}", templates::template_name(template)),
            ));
        }
        for (line, message) in hits {
            ctx.diagnostic(
                Diagnostic::error(message).with_location(files::located(loaded.path.clone(), line)),
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
            templates("Template: debconf/ok\nType: boolean\nDefault: true\n"),
            templates("Template: debconf/ok\nType: boolean\nDefault: false\n"),
            templates("Template: debconf/none\nType: boolean\n"),
            templates("Template: debconf/string\nType: string\nDefault: lintian\n"),
            templates("Template: debconf/loose\nType: Boolean\nDefault: lintian\n"),
            templates("Template: debconf/translated\nType: boolean\n_Default: lintian\n"),
        ];
        let fail = vec![
            templates("Template: debconf/boolean-bogus-default\nType: boolean\nDefault: lintian\n"),
            templates("Template: debconf/case\nType: boolean\nDefault: True\n"),
            templates("Template: debconf/blank\nType: boolean\nDefault:\n"),
            templates("Type: boolean\nDefault: yes\n").udeb(),
        ];
        BinaryPackageTester::new(BooleanTemplateHasBogusDefault, pass, fail).test_and_snapshot();
    }
}
