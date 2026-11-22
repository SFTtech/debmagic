use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct TransitionalPackageNotOldlibsOptional;

declare_rule! {
    /// A transitional package (its Description contains `transitional
    /// package`, any case) is not `oldlibs/optional`. Extra is
    /// `SECTION/PRIORITY` with the Section's fraction (the part after the
    /// first `/`, or the whole Section) and the unfolded Priority, empty when
    /// absent. Reads the Installation control, and is skipped without a
    /// Section. udebs are not checked. Warning.
    TransitionalPackageNotOldlibsOptional,
    code = "LN1239",
    tag = "transitional-package-not-oldlibs-optional",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for TransitionalPackageNotOldlibsOptional {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let fields = ctx.installation_control().fields();
        let Some(section) = fields.unfolded_value("Section") else {
            return;
        };
        let transitional = fields.get("Description").is_some_and(|description| {
            description
                .to_ascii_lowercase()
                .contains("transitional package")
        });
        if !transitional {
            return;
        }
        // Lintian splits off the archive part (`non-free/...`) first.
        let fraction = section
            .split_once('/')
            .map_or(section.as_str(), |(_, rest)| rest);
        let priority = fields.unfolded_value("Priority").unwrap_or_default();
        if priority == "optional" && fraction == "oldlibs" {
            return;
        }
        ctx.diagnostic(Diagnostic::warn(format!("{fraction}/{priority}")));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn transitional(section: &str, priority: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .field("Section", section)
            .field("Priority", priority)
            .field(
                "Description",
                "transitional package\n This is a transitional package.",
            )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            transitional("oldlibs", "optional"),
            transitional("non-free/oldlibs", "optional"),
            // Not transitional.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Section", "devel")
                .field("Priority", "standard")
                .field("Description", "a library"),
            // No Section, so nothing is checked.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Priority", "standard")
                .field("Description", "Transitional Package"),
            // udebs are not checked.
            transitional("devel", "standard").udeb(),
        ];
        let fail = vec![
            transitional("devel", "standard"),
            transitional("oldlibs", "standard"),
            transitional("non-free/oldlibs", "extra"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Section", "oldlibs")
                .field("Description", "TRANSITIONAL PACKAGE for foo"),
        ];
        BinaryPackageTester::new(TransitionalPackageNotOldlibsOptional, pass, fail)
            .test_and_snapshot();
    }
}
