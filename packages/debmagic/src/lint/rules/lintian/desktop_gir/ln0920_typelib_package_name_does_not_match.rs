use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::gir_paths::{location, triplet_or_placeholder, typelib_files, typelib_package_stem};
use crate::lint::file_tree::file_name_lossy;

pub struct TypelibPackageNameDoesNotMatch;

declare_rule! {
    /// A typelib is not in a package named `gir1.2-<stem>`.
    ///
    /// Each typelib in `usr/lib/girepository-1.0` or in
    /// `usr/lib/<triplet>/girepository-1.0` expects the package
    /// `gir1.2-` and its lowercased name without `.typelib`, with `_` as `-`.
    /// The Binary package passes when it has that name or provides it. Extra
    /// is the expected name; the typelib is the Location.
    TypelibPackageNameDoesNotMatch,
    code = "LN0920",
    tag = "typelib-package-name-does-not-match",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for TypelibPackageNameDoesNotMatch {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let (architecture, provides) = {
            let fields = ctx.installation_control().fields();
            (
                fields.unfolded_value("Architecture").unwrap_or_default(),
                fields.relation("Provides").unwrap_or_default(),
            )
        };
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = typelib_files(files, triplet_or_placeholder(&architecture))
            .into_iter()
            .filter_map(|typelib| {
                let expected = typelib_package_stem(&file_name_lossy(typelib.path()));
                let missing = package != expected && !provides.satisfies(&expected);
                missing.then(|| (location(typelib), expected))
            })
            .collect();
        for (location, expected) in hits {
            ctx.diagnostic(Diagnostic::warn(expected).with_location(location));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn package(name: &str, path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new("<typelib/>"))])
            .package(name)
            .field("Architecture", "amd64")
    }

    #[test]
    fn test() {
        let pass = vec![
            package(
                "gir1.2-bad-23",
                "usr/lib/x86_64-linux-gnu/girepository-1.0/Bad-23.typelib",
            ),
            package("gir1.2-bad-23", "usr/lib/girepository-1.0/Bad-23.typelib"),
            package("gir1.2-bad-23", "usr/lib/girepository-1.0/Bad_23.typelib"),
            package("libbad", "usr/lib/girepository-1.0/Bad-23.typelib")
                .field("Provides", "gir1.2-bad-23"),
            package("gir1.2-bad-23", "usr/share/gir-1.0/Bad-23.gir"),
        ];
        let fail = vec![
            package("libbad", "usr/lib/girepository-1.0/Bad-23.typelib"),
            package(
                "libbad",
                "usr/lib/x86_64-linux-gnu/girepository-1.0/BadExtras-23.typelib",
            ),
        ];
        BinaryPackageTester::new(TypelibPackageNameDoesNotMatch, pass, fail).test_and_snapshot();
    }
}
