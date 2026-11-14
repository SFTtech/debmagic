use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

use super::known_fields::{self, dsc_names_from_source_paragraph};

pub struct UnknownField;

declare_rule! {
    /// A control field is not in lintian's known-field list.
    ///
    /// On a Source package the names are the Dsc fields. On a Source tree they
    /// are the names `dpkg-source` would copy into the Dsc (`XS-Comment` becomes
    /// `Comment`; unknown unprefixed fields are dropped). A Binary package uses
    /// Installation control, with the udeb list when the package is a udeb.
    /// Extra is the field name. Location is empty.
    UnknownField,
    code = "LN0124",
    tag = "unknown-field",
    default_selected = true,
    experimental = false,
}

impl SourceRule for UnknownField {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let names = if let Some(dsc) = ctx.dsc() {
            dsc.fields().names().map(str::to_string).collect::<Vec<_>>()
        } else if let Some(control) = ctx.debian_control() {
            dsc_names_from_source_paragraph(control.source().names())
        } else {
            return;
        };
        for name in names {
            if !known_fields::source_known(&name) {
                ctx.diagnostic(Diagnostic::warn(name));
            }
        }
    }
}

impl BinaryPackageRule for UnknownField {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let known = if ctx.is_udeb() {
            known_fields::udeb_known
        } else {
            known_fields::binary_known
        };
        let names: Vec<String> = ctx
            .installation_control()
            .fields()
            .names()
            .map(str::to_string)
            .filter(|name| !known(name))
            .collect();
        for name in names {
            ctx.diagnostic(Diagnostic::warn(name));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{
        BinaryPackageCase, BinaryPackageTester, SourcePackageCase, SourcePackageTester,
        SourceTreeTester, TestFile,
    };

    use super::*;

    fn control(extra: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: Jane <jane@example.com>\n\
                 Standards-Version: 4.7.2\n{extra}\n\
                 Package: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            control(""),
            control("XS-Autobuild: no\nPython-Version: 2.7\n"),
        ];
        let fail = vec![
            control("XS-Comment: a note\n"),
            control("XS-Essential: no\n"),
            control("XS-../foo: bar\n"),
        ];
        SourceTreeTester::new(UnknownField, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let dsc = Dsc::parse(
            "Format: 3.0 (native)\nSource: example\nVersion: 1.0\n\
             Maintainer: Jane <jane@example.com>\nComment: a note\n",
        )
        .expect("dsc");
        let pass = vec![SourcePackageCase::from(control(""))];
        let fail = vec![SourcePackageCase::from(control("")).dsc(dsc)];
        SourcePackageTester::new(UnknownField, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_binary() {
        let pass =
            vec![BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Essential", "yes")];
        let fail = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("../bar", "moo"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Weird", "yes")
                .udeb(),
        ];
        BinaryPackageTester::new(UnknownField, pass, fail).test_and_snapshot_with_suffix("binary");
    }
}
