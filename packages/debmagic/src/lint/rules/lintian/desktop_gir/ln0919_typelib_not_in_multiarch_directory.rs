use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::gir_paths::{location, old_typelib_files, triplet_or_placeholder};

pub struct TypelibNotInMultiarchDirectory;

declare_rule! {
    /// A typelib is in `usr/lib/girepository-1.0` rather than the multiarch directory.
    ///
    /// Every direct child of `usr/lib/girepository-1.0` is a hit, whatever its
    /// name. Extra is `usr/lib/<triplet>/girepository-1.0`, where the triplet is
    /// the Architecture's `DEB_HOST_MULTIARCH` (`${DEB_HOST_MULTIARCH}` when it
    /// has none). The typelib is the Location.
    TypelibNotInMultiarchDirectory,
    code = "LN0919",
    tag = "typelib-not-in-multiarch-directory",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for TypelibNotInMultiarchDirectory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let architecture = ctx
            .installation_control()
            .fields()
            .unfolded_value("Architecture")
            .unwrap_or_default();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let extra = format!(
            "usr/lib/{}/girepository-1.0",
            triplet_or_placeholder(&architecture)
        );
        let hits: Vec<_> = old_typelib_files(files).into_iter().map(location).collect();
        for hit in hits {
            ctx.diagnostic(Diagnostic::warn(extra.clone()).with_location(hit));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn package(arch: &str, path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new("<typelib/>"))])
            .package("libbad")
            .field("Architecture", arch)
    }

    #[test]
    fn test() {
        let pass = vec![
            package(
                "amd64",
                "usr/lib/x86_64-linux-gnu/girepository-1.0/Bad-23.typelib",
            ),
            package("amd64", "usr/share/gir-1.0/Bad-23.gir"),
            package("amd64", "usr/lib/x86_64-linux-gnu/Bad.so"),
        ];
        let fail = vec![
            package("amd64", "usr/lib/girepository-1.0/Bad-23.typelib"),
            package("all", "usr/lib/girepository-1.0/Bad-23.typelib"),
        ];
        BinaryPackageTester::new(TypelibNotInMultiarchDirectory, pass, fail).test_and_snapshot();
    }
}
