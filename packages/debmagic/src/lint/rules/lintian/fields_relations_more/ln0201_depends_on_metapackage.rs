use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::BINARY_DEP_FIELDS;
use super::metapackages::is_known_metapackage;

pub struct DependsOnMetapackage;

declare_rule! {
    /// A dependency alternative names a metapackage, and this package is not one.
    ///
    /// `Depends`, `Pre-Depends`, `Recommends`, and `Suggests`. Skipped when
    /// this package is itself listed in `fields/metapackages`, or when it is
    /// transitional or a meta package (Description, Section, or a `task-`
    /// name). Extra is the field and the alternative as written. Applies to
    /// debs and udebs.
    DependsOnMetapackage,
    code = "LN0201",
    tag = "depends-on-metapackage",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DependsOnMetapackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let messages = {
            let fields = ctx.installation_control().fields();
            if super::is_transitional_or_meta(&package, fields) || is_known_metapackage(&package) {
                Vec::new()
            } else {
                BINARY_DEP_FIELDS
                    .iter()
                    .filter_map(|field| Some((*field, fields.relation(field)?)))
                    .flat_map(|(field, relation)| {
                        relation
                            .alternatives()
                            .filter(|alternative| is_known_metapackage(alternative.package()))
                            .map(|alternative| format!("{field}: {}", alternative.text()))
                            .collect::<Vec<_>>()
                    })
                    .collect()
            }
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn fields(items: &[(&str, &str)]) -> BinaryPackageCase {
        items.iter().fold(
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            |case, (name, value)| case.field(*name, *value),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            fields(&[]),
            fields(&[("Depends", "libc6")]),
            fields(&[("Conflicts", "xorg")]),
            fields(&[("Depends", "xorg-dev")]).package("xorg"),
            fields(&[("Depends", "xorg")]).field("Description", "A transitional package"),
            fields(&[("Depends", "xorg")]).field("Description", "This is a metapackage"),
            fields(&[("Depends", "xorg")]).field("Section", "metapackages"),
            fields(&[("Depends", "xorg")]).field("Section", "contrib/tasks"),
            fields(&[("Depends", "xorg")]).package("task-desktop"),
        ];
        let fail = vec![
            fields(&[("Depends", "xorg")]),
            fields(&[("Pre-Depends", "xorg")]),
            fields(&[
                ("Depends", "xorg (>= 1), foo | x-window-system"),
                ("Pre-Depends", "qt5-default"),
                ("Suggests", "mono-complete"),
            ]),
            fields(&[("Pre-Depends", "xorg")]).udeb(),
        ];
        BinaryPackageTester::new(DependsOnMetapackage, pass, fail).test_and_snapshot();
    }
}
