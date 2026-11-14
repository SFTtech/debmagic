use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct EssentialInSourcePackage;

declare_rule! {
    /// The source package declares an `Essential` field.
    ///
    /// Any value, even an empty one, counts. Source tree: `Essential` on the
    /// source paragraph of `debian/control`, or `XS-Essential` when
    /// `Essential` itself is absent (`XS-` is how the field reaches the Dsc).
    /// An `Essential` in a binary paragraph is not this Tag. Source package:
    /// the Dsc. Extra is empty.
    EssentialInSourcePackage,
    code = "LN0667",
    tag = "essential-in-source-package",
    default_selected = true,
    experimental = false,
}

impl SourceRule for EssentialInSourcePackage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let declared = match ctx.dsc() {
            Some(dsc) => dsc.fields().declares("Essential"),
            None => ctx.debian_control().is_some_and(|control| {
                control.source().declares("Essential") || control.source().declares("XS-Essential")
            }),
        };
        if declared {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(source_line: &str, binary_line: &str) -> Vec<(&'static str, TestFile)> {
        let source_line = if source_line.is_empty() {
            String::new()
        } else {
            format!("{source_line}\n")
        };
        let binary_line = if binary_line.is_empty() {
            String::new()
        } else {
            format!("{binary_line}\n")
        };
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: Example <ex@example.com>\n\
                 Standards-Version: 4.7.2\nSection: devel\n{source_line}\
                 \nPackage: example\nArchitecture: all\n{binary_line}Description: example\n extra\n"
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
            control("", ""),
            // A binary paragraph may be essential.
            control("", "Essential: yes"),
        ];
        let fail = vec![
            control("Essential: yes", ""),
            control("Essential: no", ""),
            control("XS-Essential: yes", ""),
            control("Essential:", ""),
        ];
        SourceTreeTester::new(EssentialInSourcePackage, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let bare = control("", "");
        let pass = vec![
            SourcePackageCase::from(bare.clone()).dsc(dsc("Source: example\n")),
            SourcePackageCase::from(control("XS-Essential: yes", "")).dsc(dsc("Source: example\n")),
        ];
        let fail = vec![
            SourcePackageCase::from(bare.clone()).dsc(dsc("Essential: yes\n")),
            SourcePackageCase::from(bare).dsc(dsc("Essential: no\n")),
        ];
        SourcePackageTester::new(EssentialInSourcePackage, pass, fail).test_and_snapshot();
    }
}
