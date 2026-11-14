use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

use super::SOURCE_RELATION_FIELDS;

pub struct DependsOnPackagingDev;

declare_rule! {
    /// A relation names `packaging-dev`.
    ///
    /// Source: every build-dependency and build-conflict field of the source
    /// paragraph. Binary: `Depends`, `Pre-Depends`, and `Recommends`, skipped
    /// for transitional and meta packages. `Suggests` does not count. Extra is
    /// the field name. Applies to debs and udebs.
    DependsOnPackagingDev,
    code = "LN0202",
    tag = "depends-on-packaging-dev",
    default_selected = true,
    experimental = false,
}

/// Binary fields the Check accepts for this Tag, in field-loop order.
const BINARY_FIELDS: &[&str] = &["Depends", "Pre-Depends", "Recommends"];

impl SourceRule for DependsOnPackagingDev {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = ctx
            .debian_control()
            .map(|control| field_names(control.source(), SOURCE_RELATION_FIELDS))
            .unwrap_or_default();
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

impl BinaryPackageRule for DependsOnPackagingDev {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let messages = {
            let fields = ctx.installation_control().fields();
            if super::is_transitional_or_meta(&package, fields) {
                Vec::new()
            } else {
                field_names(fields, BINARY_FIELDS)
            }
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

fn field_names(fields: &ControlParagraph, names: &[&str]) -> Vec<String> {
    names
        .iter()
        .filter_map(|field| Some((*field, fields.relation(field)?)))
        .flat_map(|(field, relation)| {
            relation
                .alternatives()
                .filter(|alternative| alternative.package() == "packaging-dev")
                .map(|_| field.to_string())
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(fields: &[(&str, &str)]) -> Vec<(&'static str, TestFile)> {
        let mut body = String::from("Source: example\n");
        for (name, value) in fields {
            body.push_str(name);
            body.push_str(": ");
            body.push_str(value);
            body.push('\n');
        }
        body.push_str("\nPackage: example\nArchitecture: all\nDescription: example\n extra\n");
        vec![("debian/control", TestFile::new(body))]
    }

    fn binary(items: &[(&str, &str)]) -> BinaryPackageCase {
        items.iter().fold(
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            |case, (name, value)| case.field(*name, *value),
        )
    }

    #[test]
    fn test() {
        let installable_only = vec![(
            "debian/control",
            TestFile::new(
                "Source: example\nBuild-Depends: debhelper\n\n\
                 Package: example\nArchitecture: all\nDepends: packaging-dev\n\
                 Description: example\n extra\n",
            ),
        )];
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            control(&[("Build-Depends", "debhelper, packaging-dev-foo")]),
            installable_only,
        ];
        let fail = vec![control(&[
            (
                "Build-Depends",
                "foo | packaging-dev, packaging-dev (>= 1) [amd64]",
            ),
            ("Build-Depends-Indep", "packaging-dev"),
            ("Build-Conflicts", "packaging-dev"),
        ])];
        SourceTreeTester::new(DependsOnPackagingDev, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_binary() {
        let pass = vec![
            binary(&[]),
            binary(&[("Suggests", "packaging-dev")]),
            binary(&[("Conflicts", "packaging-dev")]),
            binary(&[("Depends", "debhelper")]),
            binary(&[("Depends", "packaging-dev")]).field("Description", "transitional package"),
            binary(&[("Depends", "packaging-dev")]).field("Section", "metapackages"),
            binary(&[("Depends", "packaging-dev")]).package("task-desktop"),
        ];
        let fail = vec![
            binary(&[
                ("Depends", "packaging-dev"),
                ("Pre-Depends", "packaging-dev"),
                ("Recommends", "other | packaging-dev"),
            ]),
            binary(&[("Depends", "packaging-dev")]).udeb(),
        ];
        BinaryPackageTester::new(DependsOnPackagingDev, pass, fail)
            .test_and_snapshot_with_suffix("binary");
    }
}
