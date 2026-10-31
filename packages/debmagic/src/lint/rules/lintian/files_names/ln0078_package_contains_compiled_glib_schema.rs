use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::flag::flag_name;
use super::names::NameHit;
use crate::lint::rule::Severity;

pub struct PackageContainsCompiledGlibSchema;

declare_rule! {
    /// The package ships a compiled GLib schema cache.
    ///
    /// The path is `usr/share/glib-<version>/schemas/gschemas.compiled`.
    /// `gschemas.compiled` anywhere else is left alone. Udebs are checked
    /// too. Extra is empty; the path is the Location.
    PackageContainsCompiledGlibSchema,
    code = "LN0078",
    tag = "package-contains-compiled-glib-schema",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageContainsCompiledGlibSchema {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_name(ctx, NameHit::GlibSchema, Severity::Error);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::new("x")))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            tree(&[
                "usr/share/foo/gschemas.compiled",
                "usr/share/glib-2.0/gschemas.compiled",
            ]),
        ];
        let fail = vec![
            tree(&["usr/share/glib-2.0/schemas/gschemas.compiled"]),
            tree(&["usr/share/glib-2.0/schemas/gschemas.compiled"]).udeb(),
        ];
        BinaryPackageTester::new(PackageContainsCompiledGlibSchema, pass, fail).test_and_snapshot();
    }
}
