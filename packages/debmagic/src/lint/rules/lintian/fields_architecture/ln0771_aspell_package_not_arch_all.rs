use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::first_architecture;

pub struct AspellPackageNotArchAll;

declare_rule! {
    /// The Binary package is an aspell dictionary but not Architecture: all.
    ///
    /// The name is `aspell-` followed by two lowercase letters, then either
    /// the end or a `-` and more. Reads the first Architecture value of the
    /// Installation control; a missing field is not reported. Extra is empty.
    /// Applies to debs and udebs.
    AspellPackageNotArchAll,
    code = "LN0771",
    tag = "aspell-package-not-arch-all",
    default_selected = true,
    experimental = false,
}

/// `^aspell-[a-z]{2}(?:-.*)?$`.
fn is_aspell_dictionary(name: &str) -> bool {
    let Some(language) = name.strip_prefix("aspell-") else {
        return false;
    };
    let bytes = language.as_bytes();
    bytes.len() >= 2
        && bytes[..2].iter().all(u8::is_ascii_lowercase)
        && (bytes.len() == 2 || bytes[2] == b'-')
}

impl BinaryPackageRule for AspellPackageNotArchAll {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if !is_aspell_dictionary(ctx.package_name()) {
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
            case("aspell-en", "all"),
            // Not a dictionary name: two letters, then end or `-`.
            case("aspell", "any"),
            case("aspell-english", "any"),
            case("aspell-e", "any"),
            case("aspell-EN", "any"),
            case("aspell-en1", "any"),
            case("libaspell-en", "any"),
            // First value decides, as lintian reads it.
            case("aspell-en", "all any"),
            // No Architecture field: nothing to compare.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).package("aspell-en"),
        ];
        let fail = vec![
            case("aspell-en", "any"),
            case("aspell-de-extra", "amd64"),
            case("aspell-en-extra", "any"),
            case("aspell-en", "any all"),
            // Udeb.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .package("aspell-en")
                .field("Architecture", "any")
                .udeb(),
        ];
        BinaryPackageTester::new(AspellPackageNotArchAll, pass, fail).test_and_snapshot();
    }
}
