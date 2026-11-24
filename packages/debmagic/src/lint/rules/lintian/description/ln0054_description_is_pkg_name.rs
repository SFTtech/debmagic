use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::description_field::description;

pub struct DescriptionIsPkgName;

declare_rule! {
    /// The synopsis is just the package name.
    ///
    /// The lower-cased package name with `-` and `_` as spaces equals the
    /// lower-cased synopsis with `-`, `_`, `/` and `\` as spaces and runs of
    /// whitespace collapsed. Not checked when the synopsis is blank. Extra is
    /// the synopsis. Location is empty. Udebs are checked too.
    DescriptionIsPkgName,
    code = "LN0054",
    tag = "description-is-pkg-name",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DescriptionIsPkgName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(description) = description(ctx) else {
            return;
        };
        let synopsis = &description.synopsis;
        if synopsis.trim().is_empty() {
            return;
        }
        let package = ctx.package_name().to_lowercase().replace(['-', '_'], " ");
        let lowered = synopsis.to_lowercase().replace(['-', '_', '/', '\\'], " ");
        let normalised = lowered.split_whitespace().collect::<Vec<_>>().join(" ");
        // Perl collapses runs of whitespace but keeps one at either end.
        let edge = |at_start: bool| {
            let probe = if at_start {
                lowered.chars().next()
            } else {
                lowered.chars().last()
            };
            probe.is_some_and(char::is_whitespace)
        };
        let mut candidate = normalised;
        if edge(true) {
            candidate.insert(0, ' ');
        }
        if edge(false) {
            candidate.push(' ');
        }
        if package == candidate {
            ctx.diagnostic(Diagnostic::error(synopsis.clone()));
        }
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
            description("example tool"),
            description("examples"),
        ];
        let fail = vec![
            description("example"),
            description("Example"),
            description("EXAMPLE").package("example"),
            description("my/pkg").package("my-pkg"),
            description("my_pkg").package("my-pkg"),
            description("my   pkg").package("my-pkg"),
        ];
        BinaryPackageTester::new(DescriptionIsPkgName, pass, fail).test_and_snapshot();
    }
}
