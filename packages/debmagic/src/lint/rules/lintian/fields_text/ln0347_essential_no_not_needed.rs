use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

pub struct EssentialNoNotNeeded;

declare_rule! {
    /// `Essential` is exactly `no`, which is the same as omitting the field.
    ///
    /// Any other value, including `yes` and `Yes`, is not this Tag. Source
    /// tree: `Essential` on the source paragraph, or `XS-Essential` when
    /// `Essential` itself is absent (`XS-` is how the field reaches the Dsc).
    /// Source package: the Dsc. Binary package: Installation control. Extra is
    /// empty. Udebs are checked too.
    EssentialNoNotNeeded,
    code = "LN0347",
    tag = "essential-no-not-needed",
    default_selected = true,
    experimental = false,
}

impl SourceRule for EssentialNoNotNeeded {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let value = match ctx.dsc() {
            Some(dsc) => dsc.fields().unfolded_value("Essential"),
            None => ctx
                .debian_control()
                .and_then(|control| source_essential(control.source())),
        };
        if value.as_deref() == Some("no") {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

impl BinaryPackageRule for EssentialNoNotNeeded {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let value = ctx
            .installation_control()
            .fields()
            .unfolded_value("Essential");
        if value.as_deref() == Some("no") {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

/// `Essential`, or `XS-Essential` when the source paragraph only has the
/// Dsc-bound form.
fn source_essential(fields: &ControlParagraph) -> Option<String> {
    if fields.declares("Essential") {
        fields.unfolded_value("Essential")
    } else if fields.declares("XS-Essential") {
        fields.unfolded_value("XS-Essential")
    } else {
        None
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

    fn control(line: &str) -> Vec<(&'static str, TestFile)> {
        let line = if line.is_empty() {
            String::new()
        } else {
            format!("{line}\n")
        };
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: Example <ex@example.com>\n\
                 Standards-Version: 4.7.2\nSection: devel\n{line}\
                 \nPackage: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    fn dsc(text: &str) -> Dsc {
        Dsc::parse(text).expect("Dsc fixture")
    }

    #[test]
    fn test() {
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            control(""),
            control("Essential: yes"),
            control("Essential: Yes"),
            control("Essential: maybe"),
            control("XS-Essential: yes"),
        ];
        let fail = vec![control("Essential: no"), control("XS-Essential: no")];
        SourceTreeTester::new(EssentialNoNotNeeded, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let bare = control("");
        let pass = vec![
            SourcePackageCase::from(bare.clone()).dsc(dsc("Essential: yes\n")),
            SourcePackageCase::from(control("XS-Essential: no")).dsc(dsc("Essential: yes\n")),
            SourcePackageCase::from(bare.clone()).dsc(dsc("Essential: maybe\n")),
        ];
        let fail = vec![
            SourcePackageCase::from(bare).dsc(dsc("Essential: no\n")),
            SourcePackageCase::from(control("XS-Essential: yes")).dsc(dsc("Essential: no\n")),
        ];
        SourcePackageTester::new(EssentialNoNotNeeded, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_binary() {
        let essential = |value: &str| {
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Essential", value)
        };
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            essential("yes"),
            essential("Yes"),
            essential("maybe"),
            essential(""),
        ];
        let fail = vec![essential("no"), essential("no").udeb()];
        BinaryPackageTester::new(EssentialNoNotNeeded, pass, fail)
            .test_and_snapshot_with_suffix("binary");
    }
}
