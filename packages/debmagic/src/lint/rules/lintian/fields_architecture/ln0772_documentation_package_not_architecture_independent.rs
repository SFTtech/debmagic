use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::first_architecture;

pub struct DocumentationPackageNotArchitectureIndependent;

declare_rule! {
    /// The Binary package is a documentation package but not Architecture: all.
    ///
    /// The name ends in `-doc` or `-docs`. Reads the first Architecture value
    /// of the Installation control; a missing field is not reported. Extra is
    /// empty. Applies to debs and udebs.
    DocumentationPackageNotArchitectureIndependent,
    code = "LN0772",
    tag = "documentation-package-not-architecture-independent",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DocumentationPackageNotArchitectureIndependent {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let name = ctx.package_name();
        if !(name.ends_with("-doc") || name.ends_with("-docs")) {
            return;
        }
        if first_architecture(ctx).is_some_and(|architecture| architecture != "all") {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(package: &str, architecture: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .package(package)
            .field("Architecture", architecture)
    }

    #[test]
    fn test() {
        let pass = vec![
            case("foo-doc", "all"),
            case("foo-docs", "all"),
            // Not a documentation name.
            case("foo", "any"),
            case("foo-document", "any"),
            case("foo-doc-tools", "any"),
            case("foo-dev", "any"),
            // First value decides, as lintian reads it.
            case("foo-doc", "all any"),
            // No Architecture field: nothing to compare.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).package("foo-doc"),
        ];
        let fail = vec![
            case("foo-doc", "any"),
            case("foo-docs", "amd64"),
            case("foo-doc", "any all"),
            // Udeb.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .package("foo-docs")
                .field("Architecture", "amd64")
                .udeb(),
        ];
        BinaryPackageTester::new(DocumentationPackageNotArchitectureIndependent, pass, fail)
            .test_and_snapshot();
    }
}
