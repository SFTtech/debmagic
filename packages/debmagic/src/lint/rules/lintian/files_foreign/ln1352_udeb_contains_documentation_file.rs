use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct UdebContainsDocumentationFile;

declare_rule! {
    /// A udeb ships a file under `usr/share/doc/` or `usr/share/info/`.
    ///
    /// Any entry (file, link or directory) whose path starts with one of those
    /// directories followed by a non-space character is reported. Only udebs
    /// are checked. Extra is empty; the entry's path is the Location. Reported
    /// at error severity.
    UdebContainsDocumentationFile,
    code = "LN1352",
    tag = "udeb-contains-documentation-file",
    default_selected = true,
    experimental = false,
}

static DOCUMENTATION_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/share/(?:doc|info)/\S").expect("valid regex"));

impl BinaryPackageRule for UdebContainsDocumentationFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if !ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| {
                entry
                    .path()
                    .to_str()
                    .is_some_and(|name| DOCUMENTATION_PATH.is_match(name))
            })
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

    fn udeb(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new("doc"))])
            .field("Architecture", "any")
            .udeb()
    }

    #[test]
    fn test() {
        let pass = vec![
            // A deb may ship documentation.
            BinaryPackageCase::from(vec![("usr/share/doc/example/README", TestFile::new("doc"))])
                .field("Architecture", "any"),
            udeb("usr/lib/example.so"),
            udeb("usr/share/locale/de/LC_MESSAGES/example.mo"),
        ];
        let fail = vec![
            udeb("usr/share/doc/example/README"),
            udeb("usr/share/info/example.info"),
        ];
        BinaryPackageTester::new(UdebContainsDocumentationFile, pass, fail).test_and_snapshot();
    }
}
