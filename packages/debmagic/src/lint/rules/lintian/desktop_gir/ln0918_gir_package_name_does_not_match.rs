use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::gir_paths::{gir_files, gir_package_stem, location, triplet};
use crate::lint::file_tree::file_name_lossy;

pub struct GirPackageNameDoesNotMatch;

declare_rule! {
    /// A GIR XML file is not in a package named `gir1.2-<stem>-dev`.
    ///
    /// Each GIR XML file (direct child of `usr/share/gir-1.0`, or of
    /// `usr/lib/<triplet>/gir-1.0` when the Architecture has a triplet) names
    /// the package it expects: `gir1.2-` and its lowercased name without
    /// `.gir`, with `_` as `-`, then `-dev`. The Binary package passes when it
    /// has that name or provides it with the same version. Extra is the
    /// expected name; the GIR file is the Location.
    GirPackageNameDoesNotMatch,
    code = "LN0918",
    tag = "gir-package-name-does-not-match",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for GirPackageNameDoesNotMatch {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let (architecture, version, provides) = {
            let fields = ctx.installation_control().fields();
            (
                fields.unfolded_value("Architecture").unwrap_or_default(),
                fields.unfolded_value("Version").unwrap_or_default(),
                fields.relation("Provides").unwrap_or_default(),
            )
        };
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = gir_files(files, triplet(&architecture))
            .into_iter()
            .filter_map(|gir| {
                let expected = format!("{}-dev", gir_package_stem(&file_name_lossy(gir.path())));
                let missing = package != expected
                    && !provides.satisfies(&format!("{expected} (= {version})"));
                missing.then(|| (location(gir), expected))
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
        BinaryPackageCase::from(vec![(path, TestFile::new("<gir/>"))])
            .package(name)
            .field("Architecture", "amd64")
            .field("Version", "1.0")
    }

    #[test]
    fn test() {
        let pass = vec![
            package("gir1.2-bad-23-dev", "usr/share/gir-1.0/Bad-23.gir"),
            package("gir1.2-bad-23-dev", "usr/share/gir-1.0/Bad_23.gir"),
            package("libbad", "usr/share/gir-1.0/Bad-23.gir")
                .field("Provides", "gir1.2-bad-23-dev (= 1.0)"),
            package("gir1.2-bad-23-dev", "usr/share/doc/Bad-23.gir"),
            package("libbad", "usr/lib/x86_64-linux-gnu/gir-1.0/Bad-23.gir")
                .field("Provides", "gir1.2-bad-23-dev (= 1.0)"),
        ];
        let fail = vec![
            package("gir1.2-bad-23", "usr/share/gir-1.0/Bad-23.gir"),
            package("libbad", "usr/share/gir-1.0/Bad-23.gir")
                .field("Provides", "gir1.2-bad-23-dev (= 2.0)"),
            package(
                "libbad",
                "usr/lib/x86_64-linux-gnu/gir-1.0/BadExtras-23.gir",
            ),
        ];
        BinaryPackageTester::new(GirPackageNameDoesNotMatch, pass, fail).test_and_snapshot();
    }
}
