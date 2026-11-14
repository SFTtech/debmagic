use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, templates};

pub struct DuplicateLongDescriptionInTemplate;

declare_rule! {
    /// The extended description repeats the short description.
    ///
    /// The `Description` must be two lines: the second equal to the first apart
    /// from surrounding whitespace. Extra is the template name. The Location is
    /// `Description`. Udebs are checked.
    DuplicateLongDescriptionInTemplate,
    code = "LN0691",
    tag = "duplicate-long-description-in-template",
    default_selected = true,
    experimental = false,
}

/// lintian's `/^\s*(.*?)\s*?\n\s*\1\s*$/` on the trimmed description.
fn repeats_short_description(description: &str) -> bool {
    let Some((short, extended)) = description.split_once('\n') else {
        return false;
    };
    short.trim() == extended.trim() && !extended.trim().contains('\n')
}

impl BinaryPackageRule for DuplicateLongDescriptionInTemplate {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        let Some(loaded) = templates::load_gated(files) else {
            return;
        };
        let mut hits = Vec::new();
        for template in &loaded.templates {
            if repeats_short_description(&template.value("Description")) {
                hits.push((
                    template.position("Description"),
                    templates::template_name(template),
                ));
            }
        }
        for (line, name) in hits {
            ctx.diagnostic(
                Diagnostic::warn(name).with_location(files::located(loaded.path.clone(), line)),
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
            templates("Template: debconf/ok\nType: note\nDescription: Short\n Longer text.\n"),
            templates("Template: debconf/single\nType: note\nDescription: Short\n"),
            templates("Template: debconf/three\nType: note\nDescription: Short\n Short\n Short\n"),
            templates("Template: debconf/case\nType: note\nDescription: Short\n SHORT\n"),
            templates("Template: debconf/translated\nType: note\n_Description: Short\n Short\n"),
        ];
        let fail = vec![
            templates(
                "Template: debconf/duplicate-long-desc\nType: note\nDescription: Same text\n Same text\n",
            ),
            templates("Template: debconf/spaces\nType: note\nDescription:  Same \n   Same   \n"),
            templates("Type: note\nDescription: Same\n Same\n").udeb(),
        ];
        BinaryPackageTester::new(DuplicateLongDescriptionInTemplate, pass, fail)
            .test_and_snapshot();
    }
}
