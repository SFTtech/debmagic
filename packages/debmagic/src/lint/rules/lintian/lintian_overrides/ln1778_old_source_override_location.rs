use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct OldSourceOverrideLocation;

declare_rule! {
    /// A Binary package installs a file at the old source override location.
    ///
    /// Lintian's `debian/lintian-overrides` Check reads the installed files of
    /// each Binary package, not the packaging tree. It reports the entry at
    /// `debian/source.lintian-overrides`, the old place for Source-level
    /// overrides (the preferred place is `debian/source/lintian-overrides`).
    /// Reported at pedantic severity, not selected by default, with an empty
    /// extra and the path as the Location. Directories are not reported: lintian
    /// names them with a trailing slash, so the exact name never matches.
    /// Applies to debs, udebs, and ddebs.
    ///
    /// Differs from lintian: the Tag is about the Source package in its
    /// explanation, but lintian 2.144.0 emits it only from installed files, so
    /// the Source package's own `debian/source.lintian-overrides` is not
    /// reported here.
    OldSourceOverrideLocation,
    code = "LN1778",
    tag = "old-source-override-location",
    default_selected = false,
    experimental = false,
}

const OLD_LOCATION: &str = "debian/source.lintian-overrides";

impl BinaryPackageRule for OldSourceOverrideLocation {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut paths: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| !entry.is_dir() && entry.path() == std::path::Path::new(OLD_LOCATION))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        paths.sort();
        for path in paths {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn placed(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new("tag\n"))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            placed("debian/source/lintian-overrides"),
            placed("debian/source.lintian-overrides.bak"),
            placed("usr/share/doc/example/source.lintian-overrides"),
            placed("debian/changelog"),
        ];
        let fail = vec![
            placed("debian/source.lintian-overrides"),
            placed("debian/source.lintian-overrides").udeb(),
        ];
        BinaryPackageTester::new(OldSourceOverrideLocation, pass, fail).test_and_snapshot();
    }
}
