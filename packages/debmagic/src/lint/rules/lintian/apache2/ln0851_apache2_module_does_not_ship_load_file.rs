use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct Apache2ModuleDoesNotShipLoadFile;

declare_rule! {
    /// An Apache module `usr/lib/apache2/modules/mod_<name>.so` ships no
    /// `etc/apache2/mods-available/<name>.load` file.
    ///
    /// Any entry at that path counts, directories and links included. Extra is
    /// the missing path, the Location is the module file. A module whose name
    /// lacks the `mod` prefix is looked up under its bare name, as lintian does.
    /// The web server packages are exempt.
    Apache2ModuleDoesNotShipLoadFile,
    code = "LN0851",
    tag = "apache2-module-does-not-ship-load-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for Apache2ModuleDoesNotShipLoadFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if scan::is_web_server(ctx.package_name()) {
            return;
        }
        let missing = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            scan::modules(files)
                .into_iter()
                .filter_map(|(path, module)| {
                    let (load, _) = scan::module_files(&module);
                    files.get(&load).is_none().then_some((path, load))
                })
                .collect::<Vec<_>>()
        };
        for (path, load) in missing {
            ctx.diagnostic(Diagnostic::error(load).with_location(Location {
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

    fn module(module: &str, others: &[&str]) -> BinaryPackageCase {
        let mut files = vec![(
            format!("usr/lib/apache2/modules/{module}.so"),
            TestFile::new(""),
        )];
        for other in others {
            files.push((other.to_string(), TestFile::new("LoadModule x y\n")));
        }
        BinaryPackageCase::from(files)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            module("mod_foo", &["etc/apache2/mods-available/foo.load"]),
            // A directory at the load path still counts as present.
            BinaryPackageCase::from(vec![
                (
                    "usr/lib/apache2/modules/mod_foo.so".to_string(),
                    TestFile::new(""),
                ),
                (
                    "etc/apache2/mods-available/foo.load".to_string(),
                    TestFile::directory(),
                ),
            ]),
            module("mod_foo", &["etc/apache2/mods-available/foo.load"]).package("apache2"),
        ];
        let fail = vec![
            module("mod_foo2", &[]),
            module("mod_foo2", &["etc/apache2/mods-available/foo.load"]),
            module("mod_foo", &["etc/apache2/mods-available/foo.conf"]),
        ];
        BinaryPackageTester::new(Apache2ModuleDoesNotShipLoadFile, pass, fail).test_and_snapshot();
    }
}
