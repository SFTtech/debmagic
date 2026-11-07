use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct Apache2ConfigurationFilesNeedConfSuffix;

declare_rule! {
    /// An Apache configuration snippet is not named `.conf` or `.load`.
    ///
    /// Files under `etc/apache2/{conf,site,mods}-available/`. A module `.load`
    /// file is allowed. Packages named `apache2`, `apache2.2`, or
    /// `apache2-<suffix>` are the web server itself. Extra is empty. The
    /// Location is the file.
    Apache2ConfigurationFilesNeedConfSuffix,
    code = "LN0388",
    tag = "apache2-configuration-files-need-conf-suffix",
    default_selected = true,
    experimental = false,
}

static AVAILABLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^etc/apache2/(conf|site|mods)-available/(.+)$").expect("valid regex")
});

impl BinaryPackageRule for Apache2ConfigurationFilesNeedConfSuffix {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if scan::is_web_server(ctx.package_name()) {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| entry.kind() != EntryKind::Directory)
            .filter_map(|entry| {
                let path = entry.path().to_str()?;
                if path.ends_with(".conf") {
                    return None;
                }
                let captures = AVAILABLE.captures(path)?;
                let kind = captures.get(1)?.as_str();
                let name = captures.get(2)?.as_str();
                if kind == "mods" && name.ends_with(".load") {
                    return None;
                }
                Some(entry.path().to_path_buf())
            })
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

    fn files(paths: &[&str]) -> BinaryPackageCase {
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
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            files(&["etc/apache2/conf-available/foo.conf"]),
            files(&["etc/apache2/mods-available/foo.load"]),
            files(&["etc/apache2/conf.d/foo"]),
            files(&["etc/apache2/conf-available/foo"]).package("apache2"),
            files(&["etc/apache2/conf-available/foo"]).package("apache2-bin"),
        ];
        let fail = vec![
            files(&["etc/apache2/conf-available/phpmyfoo"]),
            files(&["etc/apache2/mods-available/foo"]),
            files(&["etc/apache2/site-available/local"]),
        ];
        BinaryPackageTester::new(Apache2ConfigurationFilesNeedConfSuffix, pass, fail)
            .test_and_snapshot();
    }
}
