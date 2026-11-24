use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::ln1044_package_contains_multiple_dpi_fonts::bitmap_font_subdirs;

pub struct PackageMixesMiscAndDpiFonts;

declare_rule! {
    /// The Binary package mixes `misc` bitmap fonts with `100dpi` or `75dpi` ones.
    ///
    /// Uses the bitmap font subdirectories of the package's Installed files
    /// (see `package-contains-multiple-dpi-fonts` for the collection rules,
    /// udebs included). Reports once when `misc` is present together with any
    /// other of `100dpi`, `75dpi`, `misc`. Extra is empty and there is no
    /// Location, as lintian's package-level `hint`.
    PackageMixesMiscAndDpiFonts,
    code = "LN1045",
    tag = "package-mixes-misc-and-dpi-fonts",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageMixesMiscAndDpiFonts {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let subdirs = bitmap_font_subdirs(files);
        if subdirs.contains("misc") && subdirs.len() > 1 {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(entries: &[(&str, TestFile)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            entries
                .iter()
                .map(|(path, file)| (path.to_string(), file.clone()))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn binary() {
        let pass = vec![
            tree(&[]),
            // Misc alone is fine.
            tree(&[("usr/share/fonts/X11/misc/foo.pcf.gz", TestFile::new("x"))]),
            // Mixing a DPI directory with an unrelated one is not this Tag.
            tree(&[
                ("usr/share/fonts/X11/100dpi/foo.pcf.gz", TestFile::new("x")),
                ("usr/share/fonts/X11/Type1/foo.pfb", TestFile::new("x")),
            ]),
            // A misc directory that holds no file does not count.
            tree(&[
                ("usr/share/fonts/X11/misc", TestFile::directory()),
                ("usr/share/fonts/X11/misc/sub", TestFile::directory()),
                ("usr/share/fonts/X11/75dpi/foo.pcf.gz", TestFile::new("x")),
            ]),
            // Only the exact subdirectory name `misc` counts.
            tree(&[
                (
                    "usr/share/fonts/X11/misc-extra/foo.pcf.gz",
                    TestFile::new("x"),
                ),
                ("usr/share/fonts/X11/100dpi/foo.pcf.gz", TestFile::new("x")),
            ]),
        ];
        let fail = vec![
            tree(&[
                ("usr/share/fonts/X11/misc/foo.pcf.gz", TestFile::new("x")),
                ("usr/share/fonts/X11/100dpi/foo.pcf.gz", TestFile::new("x")),
            ]),
            tree(&[
                ("usr/share/fonts/X11/misc/foo.pcf.gz", TestFile::new("x")),
                (
                    "usr/share/fonts/X11/75dpi/foo.pcf.gz",
                    TestFile::symlink("x"),
                ),
            ]),
            // Hardlinks count.
            tree(&[
                (
                    "usr/share/fonts/X11/misc/foo.pcf.gz",
                    TestFile::new("x").hardlink(),
                ),
                ("usr/share/fonts/X11/75dpi/foo.pcf.gz", TestFile::new("x")),
            ]),
            // Udebs are checked too.
            tree(&[
                ("usr/share/fonts/X11/misc/foo.pcf.gz", TestFile::new("x")),
                ("usr/share/fonts/X11/100dpi/foo.pcf.gz", TestFile::new("x")),
            ])
            .udeb(),
        ];
        BinaryPackageTester::new(PackageMixesMiscAndDpiFonts, pass, fail).test_and_snapshot();
    }
}
