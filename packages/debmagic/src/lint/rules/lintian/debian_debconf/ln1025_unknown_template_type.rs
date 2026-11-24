use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::package_relation::PackageRelation;
use crate::lint::rule::BinaryPackageRule;

use super::{files, templates};

pub struct UnknownTemplateType;

declare_rule! {
    /// A debconf template declares a `Type` that debconf does not know.
    ///
    /// Valid types are `string`, `password`, `boolean`, `select`,
    /// `multiselect`, `note`, `error`, `title` and `text`, compared exactly
    /// after trimming. `entropy` is accepted when the package's strong
    /// dependencies, `Provides` and its own name imply `cdebconf`. A template
    /// without a `Type` is not reported here. Extra is the trimmed `Type`
    /// value, which may be empty. The Location is the `Type` line. A
    /// `templates` file that is not valid debconf Deb822 is skipped. Udebs
    /// are checked.
    UnknownTemplateType,
    code = "LN1025",
    tag = "unknown-template-type",
    default_selected = true,
    experimental = false,
}

const VALID_TYPES: &[&str] = &[
    "string",
    "password",
    "boolean",
    "select",
    "multiselect",
    "note",
    "error",
    "title",
    "text",
];

impl BinaryPackageRule for UnknownTemplateType {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(loaded) = load(ctx) else {
            return;
        };
        let mut hits = Vec::new();
        for template in &loaded.templates {
            if !template.declares("Type") {
                continue;
            }
            let found = template.value("Type");
            if VALID_TYPES.contains(&found.as_str()) {
                continue;
            }
            if found == "entropy" && strong_provides_or_self(ctx).satisfies("cdebconf") {
                continue;
            }
            hits.push((found, template.position("Type")));
        }
        for (found, line) in hits {
            ctx.diagnostic(
                Diagnostic::error(found).with_location(files::located(loaded.path.clone(), line)),
            );
        }
    }
}

fn load(ctx: &mut BinaryPackageContext<'_>) -> Option<templates::TemplatesFile> {
    let files = ctx.control_files()?;
    templates::load_gated(files)
}

/// Lintian's `alldependencies`: `Pre-Depends`, `Depends`, `Provides`, and
/// the package itself, AND-ed.
fn strong_provides_or_self(ctx: &BinaryPackageContext<'_>) -> PackageRelation {
    ctx.installation_control()
        .fields()
        .relations_and(&["Pre-Depends", "Depends", "Provides"])
        .and(&PackageRelation::parse(ctx.package_name()))
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
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            templates("Template: debconf/ok\nType: string\nDescription: Name\n"),
            templates("Template: debconf/all\nType: password\nDescription: x\n"),
            templates("Template: debconf/title\nType: title\nDescription: x\n"),
            templates("Template: debconf/text\nType:   text  \nDescription: x\n"),
            // No Type is another Tag's business.
            templates("Template: debconf/no-type\nDescription: x\n"),
            // Field names are case-insensitive, Type values are not.
            templates("template: a/b\ntype: note\nDescription: x\n"),
            // entropy is known to cdebconf users.
            templates("Template: debconf/entropy\nType: entropy\nDescription: x\n")
                .field("Depends", "cdebconf"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("templates", TestFile::symlink("missing")),
        ];
        let fail = vec![
            templates("Template: debconf/unknown\nType: lintian\nDescription: x\n"),
            templates("Template: debconf/case\nType: String\nDescription: x\n"),
            templates("Template: debconf/entropy\nType: entropy\nDescription: x\n"),
            // A cdebconf-udeb is not cdebconf.
            templates("Template: debconf/entropy\nType: entropy\n")
                .field("Depends", "cdebconf-udeb"),
            // A blank Type is declared but unknown.
            templates("Template: debconf/blank\nType:\nDescription: x\n"),
            templates("Template: debconf/udeb\nType: lintian\nDescription: x\n").udeb(),
        ];
        BinaryPackageTester::new(UnknownTemplateType, pass, fail).test_and_snapshot();
    }
}
