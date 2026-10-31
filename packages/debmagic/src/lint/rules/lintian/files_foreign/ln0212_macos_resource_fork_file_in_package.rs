use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::{emit, is_file, visit};

pub struct MacosResourceForkFileInPackage;

declare_rule! {
    /// A file whose final component matches `/._` plus a name.
    ///
    /// The character after `._` must not be `_`. Names ending in `.swp` are
    /// excluded. A root `._file` has no slash, so it is not flagged. Regular
    /// files and hardlinks only. Udebs are checked too. Extra is empty.
    MacosResourceForkFileInPackage,
    code = "LN0212",
    tag = "macos-resource-fork-file-in-package",
    default_selected = true,
    experimental = false,
}

static FORK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/\._[^_/][^/]*$").expect("valid regex"));
static SWAP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.swp$").expect("valid regex"));

impl BinaryPackageRule for MacosResourceForkFileInPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<String> = visit(files)
            .into_iter()
            .filter_map(|(entry, name)| {
                (is_file(entry) && FORK.is_match(&name) && !SWAP.is_match(&name)).then_some(name)
            })
            .collect();
        for path in hits {
            emit(ctx, path, Severity::Warning);
        }
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
            file("._NEWS.Debian", body.clone()),
            file("usr/share/doc/pkg/.__hidden", body.clone()),
            file("usr/share/doc/pkg/._", body.clone()),
            file("usr/share/doc/pkg/._file.swp", body.clone()),
            file("usr/share/doc/pkg/._.swp", body.clone()),
            file("usr/share/doc/pkg/bar._baz", body.clone()),
            file("usr/share/doc/pkg/._file", TestFile::directory()),
            file("usr/share/doc/pkg/._file", TestFile::symlink("elsewhere")),
        ];
        let fail = vec![
            file("usr/share/doc/filenames/._NEWS.Debian", body.clone()),
            file("usr/share/pkg/._stuff", body.clone()),
            file("usr/share/pkg/._f", body),
            file("usr/share/pkg/._file.swap", TestFile::new("x").hardlink()).udeb(),
        ];
        BinaryPackageTester::new(MacosResourceForkFileInPackage, pass, fail).test_and_snapshot();
    }
}
