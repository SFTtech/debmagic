use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

pub struct RedundantOriginField;

declare_rule! {
    /// `Origin` is the default, `Debian` (any case).
    ///
    /// Source tree: the source paragraph of `debian/control`. Source package:
    /// the Dsc, which is where lintian reads the source package's fields.
    /// Binary package: Installation control. Installable paragraphs are not
    /// read on a source Subject. Extra is empty. Udebs are checked too.
    RedundantOriginField,
    code = "LN0346",
    tag = "redundant-origin-field",
    default_selected = true,
    experimental = false,
}

impl SourceRule for RedundantOriginField {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let redundant = match ctx.dsc() {
            Some(dsc) => is_debian(dsc.fields()),
            None => ctx
                .debian_control()
                .is_some_and(|control| is_debian(control.source())),
        };
        if redundant {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

impl BinaryPackageRule for RedundantOriginField {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if is_debian(ctx.installation_control().fields()) {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

fn is_debian(fields: &ControlParagraph) -> bool {
    fields
        .unfolded_value("Origin")
        .is_some_and(|origin| origin.eq_ignore_ascii_case("debian"))
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{
        BinaryPackageCase, BinaryPackageTester, SourcePackageCase, SourcePackageTester,
        SourceTreeTester, TestFile,
    };

    use super::*;

    fn control(origin: &str) -> Vec<(&'static str, TestFile)> {
        let origin = if origin.is_empty() {
            String::new()
        } else {
            format!("Origin: {origin}\n")
        };
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: Example <ex@example.com>\n\
                 Standards-Version: 4.7.2\nSection: devel\n{origin}\
                 \nPackage: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    fn dsc(text: &str) -> Dsc {
        Dsc::parse(text).expect("Dsc fixture")
    }

    #[test]
    fn test() {
        let installable_only = vec![(
            "debian/control",
            TestFile::new(
                "Source: example\nOrigin: Ubuntu\n\
                 \nPackage: example\nArchitecture: all\nOrigin: debian\n\
                 Description: example\n extra\n",
            ),
        )];
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            control(""),
            control("Ubuntu"),
            control("Debianish"),
            installable_only,
        ];
        let fail = vec![control("debian"), control("Debian"), control("DEBIAN")];
        SourceTreeTester::new(RedundantOriginField, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let bare = control("");
        let pass = vec![
            SourcePackageCase::from(bare.clone()).dsc(dsc("Origin: Ubuntu\n")),
            SourcePackageCase::from(control("debian")).dsc(dsc("Origin: Ubuntu\n")),
        ];
        let fail = vec![
            SourcePackageCase::from(bare).dsc(dsc("Origin: Debian\n")),
            SourcePackageCase::from(control("Ubuntu")).dsc(dsc("Origin: debian\n")),
        ];
        SourcePackageTester::new(RedundantOriginField, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_binary() {
        let origin = |value: &str| {
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Origin", value)
        };
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            origin("Ubuntu"),
            origin(""),
        ];
        let fail = vec![
            origin("debian"),
            origin("Debian"),
            origin(" debian "),
            origin("DEBIAN").udeb(),
        ];
        BinaryPackageTester::new(RedundantOriginField, pass, fail)
            .test_and_snapshot_with_suffix("binary");
    }
}
