use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct TooManyArchitectures;

declare_rule! {
    /// A binary package lists more than one `Architecture` value.
    ///
    /// Reads the Installation control `Architecture` field of the deb, udeb,
    /// or ddeb. When it has two or more whitespace-separated values, the Rule
    /// reports them sorted and joined by a space, duplicates kept. A single
    /// value, or no field, passes. Extra is the sorted list and there is no
    /// Location.
    TooManyArchitectures,
    code = "LN1047",
    tag = "too-many-architectures",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for TooManyArchitectures {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let mut architectures: Vec<String> = ctx
            .installation_control()
            .fields()
            .get("Architecture")
            .map(|value| value.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default();
        if architectures.len() > 1 {
            architectures.sort();
            ctx.diagnostic(Diagnostic::error(architectures.join(" ")));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn arch(architecture: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Architecture", architecture)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            arch("amd64"),
            arch("all"),
            arch("any"),
            // A udeb with one value.
            arch("amd64").udeb(),
        ];
        let fail = vec![
            arch("amd64 i386"),
            // Sorted in the extra, duplicates kept.
            arch("i386 amd64 amd64"),
            arch("all amd64"),
            arch("amd64 i386 weird").udeb(),
        ];
        BinaryPackageTester::new(TooManyArchitectures, pass, fail).test_and_snapshot();
    }
}
