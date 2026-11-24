use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct NonStandardApache2ModulePackageName;

declare_rule! {
    /// An Apache module package is not named `libapache2-mod-<name>` for its
    /// `usr/lib/apache2/modules/mod_<name>.so`.
    ///
    /// The expected name is `libapache2-` followed by the module name in lower
    /// case with `_` replaced by `-`. Reported once per module file whose
    /// package name differs. Extra is `<package> != <expected>`, the Location is
    /// the module file. The web server packages are exempt.
    NonStandardApache2ModulePackageName,
    code = "LN0854",
    tag = "non-standard-apache2-module-package-name",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for NonStandardApache2ModulePackageName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if scan::is_web_server(ctx.package_name()) {
            return;
        }
        let package = ctx.package_name().to_string();
        let hits = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            scan::modules(files)
                .into_iter()
                .filter_map(|(path, module)| {
                    let expected =
                        format!("libapache2-{}", module.to_lowercase().replace('_', "-"));
                    (expected != package).then(|| (path, format!("{package} != {expected}")))
                })
                .collect::<Vec<_>>()
        };
        for (path, extra) in hits {
            ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
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

    fn module(name: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            format!("usr/lib/apache2/modules/{name}.so"),
            TestFile::new(""),
        )])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            module("mod_foo").package("libapache2-mod-foo"),
            module("mod_Foo_bar").package("libapache2-mod-foo-bar"),
            // Only the module files matter, not other files.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/libapache2-modfoo".to_string(),
                TestFile::directory(),
            )]),
            module("mod_foo").package("apache2"),
        ];
        let fail = vec![
            module("mod_foo").package("libapache2-modfoo"),
            module("mod_foo2").package("libapache2-mod-foo"),
            module("mod_foo").package("libapache2-mod-foo-2"),
        ];
        BinaryPackageTester::new(NonStandardApache2ModulePackageName, pass, fail)
            .test_and_snapshot();
    }
}
