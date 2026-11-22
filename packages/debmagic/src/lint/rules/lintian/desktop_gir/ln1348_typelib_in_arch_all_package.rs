use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct TypelibInArchAllPackage;

declare_rule! {
    /// A GObject-Introspection typelib ships in an `Architecture: all` package.
    ///
    /// Typelibs are architecture-specific, so every entry directly inside
    /// `usr/lib/girepository-1.0/` of an `Architecture: all` package is
    /// reported. Lintian looks at the multiarch `usr/lib/<triplet>/` directory
    /// too, but for `all` the triplet is unknown and that directory is never
    /// found, so it is not read here. Every child is reported (files, links
    /// and directories), as lintian's `children` does, and udebs are checked.
    /// Extra is empty; the entry's path is the Location.
    TypelibInArchAllPackage,
    code = "LN1348",
    tag = "typelib-in-arch-all-package",
    default_selected = true,
    experimental = false,
}

const OLD_TYPELIB_DIR: &str = "usr/lib/girepository-1.0";

impl BinaryPackageRule for TypelibInArchAllPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let architecture = ctx
            .installation_control()
            .fields()
            .unfolded_value("Architecture");
        if architecture.as_deref() != Some("all") {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| entry.path().parent() == Some(Path::new(OLD_TYPELIB_DIR)))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    const TYPELIB: &str = "usr/lib/girepository-1.0/Example-1.0.typelib";

    #[test]
    fn test() {
        let pass = vec![
            // Only `all` is checked.
            BinaryPackageCase::from(vec![(TYPELIB, TestFile::new("typelib"))])
                .field("Architecture", "any"),
            // Only direct children of the typelib directory.
            BinaryPackageCase::from(vec![(
                "usr/lib/girepository-1.0/sub/Example-1.0.typelib",
                TestFile::new("typelib"),
            )])
            .field("Architecture", "all"),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/README",
                TestFile::new("text"),
            )])
            .field("Architecture", "all"),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(TYPELIB, TestFile::new("typelib"))])
                .field("Architecture", "all"),
            // Udebs are checked too.
            BinaryPackageCase::from(vec![(TYPELIB, TestFile::new("typelib"))])
                .field("Architecture", "all")
                .udeb(),
        ];
        BinaryPackageTester::new(TypelibInArchAllPackage, pass, fail).test_and_snapshot();
    }
}
