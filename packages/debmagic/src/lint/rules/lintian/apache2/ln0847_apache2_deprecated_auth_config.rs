use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct Apache2DeprecatedAuthConfig;

declare_rule! {
    /// An Apache configuration file uses a deprecated access-control directive.
    ///
    /// Read: web application files under `etc/apache2/{conf,site}-available/`
    /// and `etc/apache2/conf.d/`, and the `mods-available` load and conf file of
    /// each module `.so`. Reported: a line that starts with `Order`, `Satisfy`,
    /// `Allow`, `Deny`, `<Limit>`, `</Limit>`, `<LimitExcept>` or
    /// `</LimitExcept>`. Lines inside an `<IfModule !mod_authz_core>` or
    /// `<IfVersion < 2.3>` block are skipped. Extra is the directive as written,
    /// the Location is that line. A `<LimitExcept>` line is reported twice, once
    /// per pattern, because lintian's `Limit` pattern also matches it. The web
    /// server packages are exempt.
    Apache2DeprecatedAuthConfig,
    code = "LN0847",
    tag = "apache2-deprecated-auth-config",
    default_selected = true,
    experimental = false,
}

static SKIP_OPEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<\s*IfModule.*!\s*mod_authz_core|<\s*IfVersion\s+<\s*2\.3").expect("valid regex")
});
static SKIP_CLOSE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<\s*/\s*If(?:Module|Version)").expect("valid regex"));
static WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(Order|Satisfy|Allow|Deny)(?:\s+|$)").expect("valid regex"));
static LIMIT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(</?Limit.*?>)(?:\s+|$)").expect("valid regex"));
static LIMIT_EXCEPT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(</?LimitExcept.*?>)(?:\s+|$)").expect("valid regex"));

/// `(line, directive)` for each deprecated directive in `text`, in file order.
fn deprecated_directives(text: &str) -> Vec<(u32, String)> {
    let mut hits = Vec::new();
    // Lintian's counter: positive inside a skipped block; directives are
    // reported only while it is exactly zero.
    let mut skip: i64 = 0;
    for (index, line) in text.lines().enumerate() {
        let position = u32::try_from(index + 1).unwrap_or(u32::MAX);
        if SKIP_OPEN.is_match(line) {
            skip += 1;
        }
        if skip == 0 {
            for pattern in [&*WORD, &*LIMIT, &*LIMIT_EXCEPT] {
                if let Some(captures) = pattern.captures(line) {
                    hits.push((position, captures[1].to_string()));
                }
            }
        }
        if SKIP_CLOSE.is_match(line) {
            skip -= 1;
        }
    }
    hits
}

impl BinaryPackageRule for Apache2DeprecatedAuthConfig {
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
                let Some(entry) = scan::open_entry(files, &path) else {
                    continue;
                };
                let Ok(bytes) = files.read(entry) else {
                    continue;
                };
                let text = String::from_utf8_lossy(&bytes);
                for (line, directive) in deprecated_directives(&text) {
                    hits.push((path.clone(), line, directive));
                }
            }
            hits
        };
        for (path, line, directive) in hits {
            ctx.diagnostic(Diagnostic::warn(directive).with_location(Location {
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
            case(&[(
                "etc/apache2/conf-available/foo.conf",
                "Require all granted\n",
            )]),
            case(&[(
                "etc/apache2/conf-available/foo.conf",
                "AllowOverride None\nOrderly\n",
            )]),
            case(&[(
                "etc/apache2/conf-available/foo.conf",
                "<IfModule !mod_authz_core.c>\nOrder allow,deny\n</IfModule>\n",
            )]),
            case(&[(
                "etc/apache2/conf-available/foo.conf",
                "<IfVersion < 2.3>\nDeny from all\n</IfVersion>\n",
            )]),
            // Without a module `.so` the mods-available files are never read.
            case(&[("etc/apache2/mods-available/foo.load", "Order allow,deny\n")]),
            case(&[("etc/apache2/conf-available/foo.conf", "Order allow,deny\n")])
                .package("apache2"),
        ];
        let fail = vec![
            case(&[("etc/apache2/conf-available/foo.conf", "Order allow,deny\n")]),
            case(&[(
                "etc/apache2/conf.d/foo.conf",
                "<Limit POST PUT DELETE>\nRequire all granted\n</Limit>\n",
            )]),
            case(&[(
                "etc/apache2/site-available/foo",
                "<LimitExcept GET>\nDeny from all\n</LimitExcept>\n",
            )]),
            case(&[
                ("usr/lib/apache2/modules/mod_foo.so", ""),
                ("etc/apache2/mods-available/foo.load", "Satisfy any\n"),
            ]),
        ];
        BinaryPackageTester::new(Apache2DeprecatedAuthConfig, pass, fail).test_and_snapshot();
    }
}
