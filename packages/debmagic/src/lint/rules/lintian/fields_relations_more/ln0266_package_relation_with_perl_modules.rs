use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::package_relation::Alternative;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

use super::{BINARY_RELATION_FIELDS, SOURCE_RELATION_FIELDS};

pub struct PackageRelationWithPerlModules;

declare_rule! {
    /// A relation names `perl-modules` or `perl-modules-*`.
    ///
    /// Source: every build-dependency and build-conflict field, unless the
    /// source package is `perl`. Extra is the field and the alternative as
    /// written. Binary: `Depends`, `Pre-Depends`, `Recommends`, `Suggests`,
    /// `Conflicts`, `Provides`, `Enhances`, and `Breaks` (`Replaces` may name
    /// it), unless the source package is `perl`. Extra is the field and the
    /// package name, without a version or `:arch` qualifier. Applies to debs,
    /// udebs, the Source tree, and the Source package.
    PackageRelationWithPerlModules,
    code = "LN0266",
    tag = "package-relation-with-perl-modules",
    default_selected = true,
    experimental = false,
}

impl SourceRule for PackageRelationWithPerlModules {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = ctx
            .debian_control()
            .map(|control| {
                let fields = control.source();
                if is_perl(fields.get("Source")) {
                    Vec::new()
                } else {
                    source_messages(fields)
                }
            })
            .unwrap_or_default();
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

impl BinaryPackageRule for PackageRelationWithPerlModules {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.source_name() == "perl" {
            return;
        }
        let messages = binary_messages(ctx.installation_control().fields());
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

fn is_perl(name: Option<&str>) -> bool {
    name.is_some_and(|name| name.split_whitespace().next() == Some("perl"))
}

fn is_perl_modules(alternative: &Alternative) -> bool {
    alternative.package().starts_with("perl-modules")
}

fn source_messages(fields: &ControlParagraph) -> Vec<String> {
    SOURCE_RELATION_FIELDS
        .iter()
        .filter_map(|field| Some((*field, fields.relation(field)?)))
        .flat_map(|(field, relation)| {
            relation
                .alternatives()
                .filter(|alternative| is_perl_modules(alternative))
                .map(|alternative| format!("{field}: {}", alternative.text()))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn binary_messages(fields: &ControlParagraph) -> Vec<String> {
    BINARY_RELATION_FIELDS
        .iter()
        .filter(|field| **field != "Replaces")
        .filter_map(|field| Some((*field, fields.relation(field)?)))
        .flat_map(|(field, relation)| {
            relation
                .alternatives()
                .filter(|alternative| is_perl_modules(alternative))
                .map(|alternative| format!("{field}: {}", alternative.package()))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(source: &str, fields: &[(&str, &str)]) -> Vec<(&'static str, TestFile)> {
        let mut body = format!("Source: {source}\n");
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
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            control("example", &[("Build-Depends", "perl, libperl-modules")]),
            control(
                "perl",
                &[("Build-Depends", "perl-modules, perl-modules-5.22")],
            ),
            control("perl (1.0)", &[("Build-Depends", "perl-modules")]),
        ];
        let fail = vec![control(
            "example",
            &[
                ("Build-Depends", "perl-modules, perl-modules-5.22 (>= 5.22)"),
                ("Build-Depends-Indep", "foo | perl-modules:any"),
                ("Build-Conflicts", "perl-modules"),
            ],
        )];
        SourceTreeTester::new(PackageRelationWithPerlModules, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_binary() {
        let pass = vec![
            binary(&[]),
            binary(&[("Depends", "perl")]),
            binary(&[("Replaces", "perl-modules")]),
            binary(&[("Depends", "perl-modules")]).field("Source", "perl"),
            binary(&[("Depends", "perl-modules")]).field("Source", "perl (5.40)"),
            binary(&[("Depends", "libperl-modules")]),
        ];
        let fail = vec![
            binary(&[
                ("Depends", "perl-modules (>= 5)"),
                ("Pre-Depends", "perl-modules-5.22"),
            ]),
            binary(&[("Conflicts", "foo | perl-modules:any")]),
            binary(&[("Provides", "perl-modules")]),
            binary(&[("Depends", "perl-modules")])
                .package("perl")
                .field("Source", "not-perl"),
            binary(&[("Suggests", "perl-modules")]).udeb(),
        ];
        BinaryPackageTester::new(PackageRelationWithPerlModules, pass, fail)
            .test_and_snapshot_with_suffix("binary");
    }
}
