use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind};
use crate::lint::rule::BinaryPackageRule;

pub struct IncorrectNamingOfPkcs11Module;

declare_rule! {
    /// An installed path below `usr/share/p11-kit/modules/` does not match
    /// `[a-z0-9][a-z0-9_.-]*.module`, with the first character alphanumeric.
    ///
    /// Walks every Data files entry. A directory is matched with its trailing
    /// slash, and the directory of the modules folder itself is not matched.
    /// Extra is empty; the path is the Location. Udebs are checked too.
    IncorrectNamingOfPkcs11Module,
    code = "LN0823",
    tag = "incorrect-naming-of-pkcs11-module",
    default_selected = true,
    experimental = false,
}

static MODULES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/share/p11-kit/modules/.").expect("valid regex"));
static MODULE_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/share/p11-kit/modules/[[:alnum:]][[:alnum:]_.-]*\.module$")
        .expect("valid regex")
});

impl BinaryPackageRule for IncorrectNamingOfPkcs11Module {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter_map(lintian_name)
            .filter(|name| is_misnamed(name))
            .map(PathBuf::from)
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

/// The path of an Entry as lintian names an installed item: a directory has a
/// trailing slash. Non-UTF-8 paths are skipped.
fn lintian_name(entry: &Entry) -> Option<String> {
    let path = entry.path().to_str()?;
    Some(if matches!(entry.kind(), EntryKind::Directory) {
        format!("{path}/")
    } else {
        path.to_string()
    })
}

/// Below the modules directory and not a well-named `*.module` file.
fn is_misnamed(name: &str) -> bool {
    MODULES.is_match(name) && !MODULE_NAME.is_match(name)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn data(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, file)])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            data("usr/share/p11-kit/modules/foo.module", TestFile::new("x")),
            data(
                "usr/share/p11-kit/modules/a_b-c.1.module",
                TestFile::new("x"),
            ),
            data("usr/share/p11-kit/modules/Foo.module", TestFile::new("x")),
            data("usr/share/p11-kit/modules", TestFile::directory()),
            // Outside the modules directory.
            data("usr/share/p11-kit/_bad.module", TestFile::new("x")),
            data("usr/share/p11-kit/modulesx/_bad", TestFile::new("x")),
        ];
        let fail = vec![
            data("usr/share/p11-kit/modules/_bad.module", TestFile::new("x")),
            data("usr/share/p11-kit/modules/-bad.module", TestFile::new("x")),
            data("usr/share/p11-kit/modules/bad.conf", TestFile::new("x")),
            data("usr/share/p11-kit/modules/bad.MODULE", TestFile::new("x")),
            data(
                "usr/share/p11-kit/modules/sub/foo.module",
                TestFile::new("x"),
            ),
            data("usr/share/p11-kit/modules/sub", TestFile::directory()),
            data("usr/share/p11-kit/modules/_bad.module", TestFile::new("x")).udeb(),
        ];
        BinaryPackageTester::new(IncorrectNamingOfPkcs11Module, pass, fail).test_and_snapshot();
    }
}
