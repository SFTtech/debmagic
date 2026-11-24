use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::alternatives_of;

pub struct DependsOnLibdb1Compat;

declare_rule! {
    /// A package depends on the obsolete `libdb1-compat`.
    ///
    /// `Pre-Depends` and `Depends`, any alternative. Skipped for packages
    /// named `libc6`, `libc6.1`, or `libc0.3` (and anything starting with
    /// those). Extra is the field. Applies to debs and udebs.
    DependsOnLibdb1Compat,
    code = "LN0573",
    tag = "depends-on-libdb1-compat",
    default_selected = true,
    experimental = false,
}

static LIBC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^libc(?:6|6.1|0.3)").expect("valid regex"));

impl BinaryPackageRule for DependsOnLibdb1Compat {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if LIBC.is_match(ctx.package_name()) {
            return;
        }
        let messages: Vec<String> = alternatives_of(
            ctx.installation_control().fields(),
            &["Depends", "Pre-Depends"],
        )
        .into_iter()
        .filter(|(_, alternative)| alternative.package() == "libdb1-compat")
        .map(|(field, _)| field)
        .collect();
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
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
            binary(&[("Depends", "libdb1-compat-dev, libdb2")]),
            binary(&[("Recommends", "libdb1-compat")]),
            binary(&[("Depends", "libdb1-compat")]).package("libc6"),
            binary(&[("Depends", "libdb1-compat")]).package("libc6.1-dev"),
        ];
        let fail = vec![
            binary(&[("Depends", "foo | libdb1-compat (>= 1)")]),
            binary(&[("Pre-Depends", "libdb1-compat:any")]),
            binary(&[("Depends", "libdb1-compat")]).package("libc0"),
            binary(&[("Depends", "libdb1-compat")]).udeb(),
        ];
        BinaryPackageTester::new(DependsOnLibdb1Compat, pass, fail).test_and_snapshot();
    }
}
