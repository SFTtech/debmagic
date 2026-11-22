use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageContainsUpstreamInstallationDocumentation;

declare_rule! {
    /// A Binary package ships upstream's `INSTALL` file.
    ///
    /// Matches `usr/share/doc/<package>/INSTALL`, optionally followed by
    /// `.` and at least one more character (`INSTALL.txt`, `INSTALL.gz`).
    /// Directories are not matched; links and other non-directory entries
    /// are, as in lintian. Extra is empty; the entry's path is the Location.
    /// Reported at warning severity.
    PackageContainsUpstreamInstallationDocumentation,
    code = "LN1351",
    tag = "package-contains-upstream-installation-documentation",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PackageContainsUpstreamInstallationDocumentation {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let prefix = format!("usr/share/doc/{}/INSTALL", ctx.package_name());
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| entry.kind() != EntryKind::Directory)
            .filter_map(|entry| {
                let name = entry.path().to_str()?;
                let rest = name.strip_prefix(prefix.as_str())?;
                (rest.is_empty() || (rest.starts_with('.') && rest.len() > 1))
                    .then(|| entry.path().to_path_buf())
            })
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
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

    fn case(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new("install"))])
            .package("example")
            .field("Architecture", "all")
    }

    #[test]
    fn test() {
        let pass = vec![
            case("usr/share/doc/example/README"),
            case("usr/share/doc/example/INSTALL/other"),
            case("usr/share/doc/other/INSTALL"),
            case("usr/share/doc/example/INSTALLATION"),
            case("usr/share/doc/example/INSTALL."),
        ];
        let fail = vec![
            case("usr/share/doc/example/INSTALL"),
            case("usr/share/doc/example/INSTALL.txt"),
            case("usr/share/doc/example/INSTALL.gz"),
        ];
        BinaryPackageTester::new(PackageContainsUpstreamInstallationDocumentation, pass, fail)
            .test_and_snapshot();
    }
}
