use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::{emit, visit};

pub struct DebugSuffixNotDbg;

declare_rule! {
    /// The package ships something under `usr/lib/debug/` but is not a debug package.
    ///
    /// One hint, on the first index name in lintian's sort (directories keep
    /// the trailing slash, and that slash sorts after `-`). A debug package
    /// is one whose Installation control `Package` name contains `-dbg` or
    /// `-dbgsym` (`-dbg` is unanchored, so `-dbg-doc` counts and `-debug`
    /// does not). Any entry kind. Udebs are checked too. Extra is empty.
    DebugSuffixNotDbg,
    code = "LN0214",
    tag = "debug-suffix-not-dbg",
    default_selected = true,
    experimental = false,
}

static DEBUG_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/lib/debug/\S").expect("valid regex"));
static DEBUG_PACKAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"-dbg(?:sym)?").expect("valid regex"));

impl BinaryPackageRule for DebugSuffixNotDbg {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if DEBUG_PACKAGE.is_match(ctx.package_name()) {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let Some(path) = visit(files)
            .into_iter()
            .find_map(|(_, name)| DEBUG_PATH.is_match(&name).then_some(name))
        else {
            return;
        };
        emit(ctx, path, Severity::Warning);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn file(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), file)])
    }

    #[test]
    fn test() {
        let body = TestFile::new("x");
        let pass = vec![
            file("usr/lib/debug", TestFile::directory()),
            file("usr/lib/debugfoo/bar", body.clone()),
            file("usr/lib/debug/bar", body.clone()).package("foo-dbg"),
            file("usr/lib/debug/bar", body.clone()).package("foo-dbgsym"),
            file("usr/lib/debug/bar", body.clone()).package("not-dbg-pkg"),
            file("usr/lib/debug/bar", body.clone()).package("foo-dbg-doc"),
        ];
        let fail = vec![
            file("usr/lib/debug/bar", body.clone()),
            file("usr/lib/debug/bar", body.clone()).package("foo-debug"),
            file("usr/lib/debug/.build-id", TestFile::directory()),
            BinaryPackageCase::from(vec![
                ("usr/lib/debug/foo".to_string(), TestFile::directory()),
                ("usr/lib/debug/foo-1".to_string(), body),
            ]),
            file("usr/lib/debug/bar", TestFile::new("x")).udeb(),
        ];
        BinaryPackageTester::new(DebugSuffixNotDbg, pass, fail).test_and_snapshot();
    }
}
