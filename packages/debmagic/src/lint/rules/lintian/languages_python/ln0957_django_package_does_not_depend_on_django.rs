use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct DjangoPackageDoesNotDependOnDjango;

declare_rule! {
    /// A `python3-django-*` package does not depend on `python3-django`.
    ///
    /// The check looks at `Pre-Depends` and `Depends` (lintian's `strong`
    /// relation) of the Binary package. The hint extra is `python3-django`,
    /// and it has no Location. Packages whose name ends in `-dev`, `-doc`,
    /// `-docs`, `-common` or `-tools` are skipped. Only `python3-django-`
    /// names are checked: a plain `python-django-*` package is not.
    DjangoPackageDoesNotDependOnDjango,
    code = "LN0957",
    tag = "django-package-does-not-depend-on-django",
    default_selected = true,
    experimental = false,
}

static DJANGO_PACKAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^python3-django-").expect("valid regex"));
static IGNORED_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(-dev$|-docs?$|-common$|-tools$)").expect("valid regex"));

const BASE_PACKAGE: &str = "python3-django";

impl BinaryPackageRule for DjangoPackageDoesNotDependOnDjango {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let name = ctx.package_name().to_string();
        if !DJANGO_PACKAGE.is_match(&name) || IGNORED_SUFFIX.is_match(&name) {
            return;
        }
        let strong = ctx
            .installation_control()
            .fields()
            .relations_and(&["Pre-Depends", "Depends"]);
        if !strong.satisfies(BASE_PACKAGE) {
            ctx.diagnostic(Diagnostic::warn(BASE_PACKAGE));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn django(package: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/lib/python3/dist-packages/django_app/__init__.py".to_string(),
            TestFile::new("x"),
        )])
        .package(package)
    }

    #[test]
    fn binary() {
        let pass = vec![
            // Pre-Depends counts as a strong dependency.
            django("python3-django-false-positive").field("Pre-Depends", "python3-django"),
            // Ignored suffixes are not checked.
            django("python3-django-dev"),
            // Other names are not Django packages.
            django("python3-other").field("Depends", "python3"),
        ];
        let fail = vec![
            django("python3-django-positive").field("Depends", "python3"),
            // A Recommends relation is not strong enough.
            django("python3-django-recommends").field("Recommends", "python3-django"),
        ];
        BinaryPackageTester::new(DjangoPackageDoesNotDependOnDjango, pass, fail)
            .test_and_snapshot();
    }
}
