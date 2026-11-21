use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct LibRecommendsDocumentation;

declare_rule! {
    /// A library package recommends a documentation package.
    ///
    /// The package name starts with `lib` and does not end in `-dev`, `-doc`,
    /// `-docs`, `-tools`, or `-bin`. An alternative of `Recommends` that ends
    /// in `-doc` or `-docs` as written is reported; the development package
    /// should recommend it instead. Extra is `Recommends` and the alternative
    /// as written. Applies to debs and udebs.
    LibRecommendsDocumentation,
    code = "LN0936",
    tag = "lib-recommends-documentation",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for LibRecommendsDocumentation {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let messages: Vec<String> = {
            let fields = ctx.installation_control().fields();
            if !is_library_package(&package) {
                Vec::new()
            } else {
                fields
                    .relation("Recommends")
                    .map(|relation| {
                        relation
                            .alternatives()
                            .filter(|alternative| is_documentation(alternative.text()))
                            .map(|alternative| format!("Recommends: {}", alternative.text()))
                            .collect()
                    })
                    .unwrap_or_default()
            }
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

/// `^lib` and not `-dev`, `-doc`, `-docs`, `-tools`, or `-bin`.
fn is_library_package(package: &str) -> bool {
    package.starts_with("lib")
        && !["-dev", "-doc", "-docs", "-tools", "-bin"]
            .iter()
            .any(|suffix| package.ends_with(suffix))
}

fn is_documentation(alternative: &str) -> bool {
    alternative.ends_with("-doc") || alternative.ends_with("-docs")
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn fields(items: &[(&str, &str)]) -> BinaryPackageCase {
        items.iter().fold(
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            |case, (name, value)| case.field(*name, *value),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            fields(&[]),
            fields(&[("Recommends", "libfoo-doc")]).package("libfoo-dev"),
            fields(&[("Recommends", "libfoo-doc")]).package("libfoo-doc"),
            fields(&[("Recommends", "libfoo-bin-doc")]).package("libfoo-tools"),
            fields(&[("Recommends", "foo-doc")]).package("foo"),
            fields(&[("Depends", "libfoo-doc")]).package("libfoo"),
            fields(&[("Recommends", "libfoo-doc (>= 1)")]).package("libfoo"),
        ];
        let fail = vec![
            fields(&[("Recommends", "libfoo-doc")]).package("libfoo"),
            fields(&[("Recommends", "libfoo-docs | libbar")]).package("libfoo3"),
        ];
        BinaryPackageTester::new(LibRecommendsDocumentation, pass, fail).test_and_snapshot();
    }
}
