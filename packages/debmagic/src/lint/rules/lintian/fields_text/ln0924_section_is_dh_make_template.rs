use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct SectionIsDhMakeTemplate;

declare_rule! {
    /// The `Section` of the Binary package is the `dh_make` placeholder
    /// `unknown` that was never changed.
    ///
    /// Reads the Installation control. Only a `Section` with no archive part
    /// (no `area/` prefix, or an empty one) whose last part is `unknown` is
    /// reported, with empty extra. Other unknown sections are not this Tag.
    /// Udebs are skipped, as lintian's Check returns early for them.
    SectionIsDhMakeTemplate,
    code = "LN0924",
    tag = "section-is-dh_make-template",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SectionIsDhMakeTemplate {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(section) = ctx
            .installation_control()
            .fields()
            .unfolded_value("Section")
        else {
            return;
        };
        let mut parts = section.splitn(2, '/');
        let first = parts.next().unwrap_or_default();
        let (division, fraction) = match parts.next() {
            Some(fraction) => (Some(first), fraction),
            None => (None, first),
        };
        if fraction == "unknown" && division.is_none_or(str::is_empty) {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn section(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Section", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            section("admin"),
            section("libs"),
            // An archive part before `unknown` is not the template.
            section("contrib/unknown"),
            section("non-free/unknown"),
            section("unknown/admin"),
            section("unknown-tool"),
            section("Unknown"),
            section("unknown").udeb(),
        ];
        let fail = vec![
            section("unknown"),
            section("/unknown"),
            section("  unknown  "),
        ];
        BinaryPackageTester::new(SectionIsDhMakeTemplate, pass, fail).test_and_snapshot();
    }
}
