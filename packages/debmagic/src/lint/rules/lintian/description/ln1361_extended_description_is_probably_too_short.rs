use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::description_field::description;

pub struct ExtendedDescriptionIsProbablyTooShort;

declare_rule! {
    /// The extended description is one line long, or two lines of which the
    /// second is not a separator.
    ///
    /// Reported when the synopsis does not mention `dummy` or `transition`,
    /// the package is not a transitional or meta package (judged from the
    /// Description and Section as lintian does) and not a debug package
    /// (`-dbg` or `-dbgsym` in its name). The extended description counts the
    /// lines after the synopsis, with trailing empty lines dropped, so a ` .`
    /// separator counts as a line. Reported with no extra. Location is empty.
    /// Udebs are skipped. Difference from lintian: a Description that is
    /// empty is `extended-description-is-empty` and not reported here.
    ExtendedDescriptionIsProbablyTooShort,
    code = "LN1361",
    tag = "extended-description-is-probably-too-short",
    default_selected = false,
    experimental = false,
}

static META_IN_DESCRIPTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)meta[ -]?package|(?:dependency|dummy|empty) package").expect("valid regex")
});

static SECTION_META: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|/)(?:tasks|metapackages)$").expect("valid regex"));

static TRANSITION_OR_DUMMY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)dummy|transition").expect("valid regex"));

impl BinaryPackageRule for ExtendedDescriptionIsProbablyTooShort {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(description) = description(ctx) else {
            return;
        };
        if description.lines.len() >= 2 || TRANSITION_OR_DUMMY.is_match(&description.synopsis) {
            return;
        }
        let fields = ctx.installation_control().fields();
        let long = fields.unfolded_value("Description").unwrap_or_default();
        let section = fields.get("Section").unwrap_or_default();
        let transitional = long.to_lowercase().contains("transitional package");
        let meta = META_IN_DESCRIPTION.is_match(&long) || SECTION_META.is_match(section);
        let debug = ctx.package_name().contains("-dbg");
        if transitional || meta || debug {
            return;
        }
        ctx.diagnostic(Diagnostic::warn("").with_severity(Severity::Info));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn description(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Description", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            description("synopsis\n One.\n Two.\n Three."),
            description("synopsis\n One line.\n ."),
            description("synopsis\n One line.\n .\n"),
            description("A transitional package\n Only one line."),
            description("A meta package\n Only one line."),
            description("dummy package\n Only one line."),
            description("synopsis\n One line.").udeb(),
        ];
        let fail = vec![
            description("synopsis\n One line."),
            description("Tools\n Only one line of text."),
        ];
        BinaryPackageTester::new(ExtendedDescriptionIsProbablyTooShort, pass, fail)
            .test_and_snapshot();
    }
}
