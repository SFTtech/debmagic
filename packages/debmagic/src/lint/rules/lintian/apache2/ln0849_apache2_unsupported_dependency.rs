use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct Apache2UnsupportedDependency;

declare_rule! {
    /// An Apache configuration file declares `# Conflicts:`, which Apache
    /// modules do not support.
    ///
    /// Read the same files as the deprecated-auth Rule, except the module
    /// `mods-available` files, where `Conflicts` is allowed. Reported: a comment
    /// `# Conflicts: …`. Extra is `Conflicts`, the Location is that line. The
    /// web server packages are exempt.
    Apache2UnsupportedDependency,
    code = "LN0849",
    tag = "apache2-unsupported-dependency",
    default_selected = true,
    experimental = false,
}

static FIELD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^#\s*(Depends|Conflicts):\s+(.*?)\s*$").expect("valid regex"));

impl BinaryPackageRule for Apache2UnsupportedDependency {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if scan::is_web_server(ctx.package_name()) {
            return;
        }
        let hits = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            let mut hits = Vec::new();
            for path in scan::inspected_conf_files(files) {
                // Module files are `mods-available`, whose conftype lintian
                // reports as `mods`; `Conflicts` is only unsupported elsewhere.
                if path.starts_with("etc/apache2/mods-available/") {
                    continue;
                }
                let Some(entry) = scan::open_entry(files, &path) else {
                    continue;
                };
                let Ok(bytes) = files.read(entry) else {
                    continue;
                };
                let text = String::from_utf8_lossy(&bytes);
                for (index, line) in text.lines().enumerate() {
                    let Some(captures) = FIELD.captures(line) else {
                        continue;
                    };
                    if &captures[1] == "Conflicts" {
                        let position = u32::try_from(index + 1).unwrap_or(u32::MAX);
                        hits.push((path.clone(), position));
                    }
                }
            }
            hits
        };
        for (path, line) in hits {
            ctx.diagnostic(Diagnostic::warn("Conflicts").with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(files: &[(&str, &str)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            files
                .iter()
                .map(|(path, content)| (path.to_string(), TestFile::new(*content)))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            case(&[("etc/apache2/conf-available/foo.conf", "# Depends: foo\n")]),
            case(&[("etc/apache2/conf-available/foo.conf", "# Conflicts:\n")]),
            // Conflicts is allowed for module load and conf files.
            case(&[
                ("usr/lib/apache2/modules/mod_foo.so", ""),
                ("etc/apache2/mods-available/foo.load", "# Conflicts: bar\n"),
            ]),
            // Without a module `.so` the mods-available files are never read.
            case(&[("etc/apache2/mods-available/foo.load", "# Conflicts: bar\n")]),
            case(&[("etc/apache2/conf-available/foo.conf", "# Conflicts: foo\n")])
                .package("apache2"),
        ];
        let fail = vec![
            case(&[(
                "etc/apache2/conf-available/phpmyfoo.conf",
                "# Depends: foo\n# Conflicts: bar\n",
            )]),
            case(&[("etc/apache2/conf.d/foo.conf", "# Conflicts: bar\n")]),
            case(&[("etc/apache2/site-available/foo", "#  Conflicts: bar\n")]),
            case(&[("etc/apache2/conf-available/foo.conf", "#Conflicts: foo\n")]),
        ];
        BinaryPackageTester::new(Apache2UnsupportedDependency, pass, fail).test_and_snapshot();
    }
}
