use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{alternatives_of, is_virtual_package};

pub struct BreaksWithoutVersion;

declare_rule! {
    /// A `Breaks` entry has no version constraint.
    ///
    /// Every alternative of `Breaks` without a `(op version)`, unless its
    /// package is a known virtual package or the package's own `Replaces`
    /// already implies that alternative. Extra is the alternative as written.
    /// Applies to debs and udebs.
    BreaksWithoutVersion,
    code = "LN0576",
    tag = "breaks-without-version",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BreaksWithoutVersion {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages = messages(ctx.installation_control().fields());
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

fn messages(fields: &ControlParagraph) -> Vec<String> {
    let replaces = fields.relation("Replaces").unwrap_or_default();
    alternatives_of(fields, &["Breaks"])
        .into_iter()
        .filter(|(_, alternative)| {
            alternative.version().is_none()
                && !is_virtual_package(alternative.package())
                && !replaces.satisfies(alternative.text())
        })
        .map(|(_, alternative)| alternative.text().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::BinaryPackageTester;

    use super::super::testing::binary;
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            binary(&[]),
            binary(&[("Breaks", "foo (<< 1.0), bar (= 2)")]),
            binary(&[("Conflicts", "foo")]),
            binary(&[("Breaks", "foo"), ("Replaces", "foo")]),
            // A versioned Replaces already implies the bare name.
            binary(&[("Breaks", "foo"), ("Replaces", "foo (<< 2)")]),
            binary(&[("Breaks", "foo"), ("Replaces", "foo (<< 2), foo")]),
            // A virtual package cannot be versioned.
            binary(&[("Breaks", "mail-transport-agent")]),
        ];
        let fail = vec![
            binary(&[("Breaks", "package-without-version, foo (<< 1) | bar")]),
            binary(&[("Breaks", "foo, bar"), ("Replaces", "foo, baz")]),
            binary(&[("Breaks", "foo:any"), ("Replaces", "other")]).udeb(),
        ];
        BinaryPackageTester::new(BreaksWithoutVersion, pass, fail).test_and_snapshot();
    }
}
