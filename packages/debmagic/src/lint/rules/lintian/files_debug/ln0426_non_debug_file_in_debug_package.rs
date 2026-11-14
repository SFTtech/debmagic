use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct NonDebugFileInDebugPackage;

declare_rule! {
    /// An automatically built debug package ships a file that is not `*.debug`.
    ///
    /// The package name matches `-dbg` or `-dbgsym`, and Installation control
    /// declares `Auto-Built-Package`. Regular files and hardlinks whose names
    /// do not end in `.debug` are reported. Extra is empty. The path is the
    /// Location. Udebs are checked too.
    NonDebugFileInDebugPackage,
    code = "LN0426",
    tag = "non-debug-file-in-debug-package",
    default_selected = true,
    experimental = false,
}

static DEBUG_PACKAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"-dbg(?:sym)?").expect("valid regex"));

impl BinaryPackageRule for NonDebugFileInDebugPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if !DEBUG_PACKAGE.is_match(ctx.package_name())
            || !ctx
                .installation_control()
                .fields()
                .declares("Auto-Built-Package")
        {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut paths = Vec::new();
        for entry in files.entries() {
            if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
                continue;
            }
            let Some(path) = entry.path().to_str() else {
                continue;
            };
            if path.ends_with(".debug") {
                continue;
            }
            paths.push(entry.path().to_path_buf());
        }
        paths.sort();
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

    fn debug(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new("bytes\n"))])
            .package("example-dbgsym")
            .field("Auto-Built-Package", "debug-symbols")
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            debug("usr/lib/debug/bin/tool.debug"),
            BinaryPackageCase::from(vec![("foo".to_string(), TestFile::new("bytes\n"))])
                .package("example-dbgsym"),
            BinaryPackageCase::from(vec![("foo".to_string(), TestFile::new("bytes\n"))])
                .field("Auto-Built-Package", "debug-symbols"),
        ];
        let fail = vec![
            debug("foo"),
            debug("usr/share/doc/example-dbgsym/copyright"),
            BinaryPackageCase::from(vec![
                ("foo".to_string(), TestFile::new("a\n")),
                (
                    "usr/lib/debug/bin/tool.debug".to_string(),
                    TestFile::new("b\n"),
                ),
            ])
            .package("libexample-dbg")
            .field("Auto-Built-Package", "debug-symbols"),
        ];
        BinaryPackageTester::new(NonDebugFileInDebugPackage, pass, fail).test_and_snapshot();
    }
}
