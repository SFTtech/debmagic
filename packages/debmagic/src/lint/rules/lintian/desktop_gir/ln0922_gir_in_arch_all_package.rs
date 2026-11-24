use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::gir_paths::{gir_files, location, triplet};

pub struct GirInArchAllPackage;

declare_rule! {
    /// A GIR XML file is in a Binary package with `Architecture: all`.
    ///
    /// Every GIR XML file (direct child of `usr/share/gir-1.0`) is a hit. The
    /// multiarch directory is not searched: `all` has no triplet. Extra is
    /// empty; the GIR file is the Location.
    GirInArchAllPackage,
    code = "LN0922",
    tag = "gir-in-arch-all-package",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for GirInArchAllPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let architecture = ctx
            .installation_control()
            .fields()
            .unfolded_value("Architecture")
            .unwrap_or_default();
        if architecture != "all" {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = gir_files(files, triplet(&architecture))
            .into_iter()
            .map(location)
            .collect();
        for hit in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(hit));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn package(arch: &str, path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new("<gir/>"))])
            .package("gir1.2-bad-23")
            .field("Architecture", arch)
    }

    #[test]
    fn test() {
        let pass = vec![
            package("amd64", "usr/share/gir-1.0/Bad-23.gir"),
            package("all", "usr/share/doc/Bad-23.gir"),
            package("all", "usr/share/gir-1.0/nested/Bad-23.gir"),
            package("all", "usr/share/gir-1.0/Bad-23.typelib"),
        ];
        let fail = vec![
            package("all", "usr/share/gir-1.0/Bad-23.gir"),
            package("all", "usr/share/gir-1.0/BadExtras-23.gir"),
        ];
        BinaryPackageTester::new(GirInArchAllPackage, pass, fail).test_and_snapshot();
    }
}
