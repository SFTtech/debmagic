use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::flag::{emit, is_file, visit};

pub struct FileDirectlyInUsrShareDoc;

declare_rule! {
    /// A file whose path is `usr/share/doc/<one component>`.
    ///
    /// Regular files and hardlinks only. The package doc directory itself
    /// is a directory, so it is not flagged, and neither is a file nested
    /// under it. Skipped for udebs. Extra is empty.
    FileDirectlyInUsrShareDoc,
    code = "LN0210",
    tag = "file-directly-in-usr-share-doc",
    default_selected = true,
    experimental = false,
}

static DIRECT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/share/doc/[^/]+$").expect("valid regex"));

impl BinaryPackageRule for FileDirectlyInUsrShareDoc {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<String> = visit(files)
            .into_iter()
            .filter_map(|(entry, name)| (is_file(entry) && DIRECT.is_match(&name)).then_some(name))
            .collect();
        for path in hits {
            emit(ctx, path, Severity::Error);
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
        let pass = vec![
            file("usr/share/doc/example", TestFile::directory()),
            file("usr/share/doc/example/README", TestFile::new("x")),
            file("usr/share/doc/bar", TestFile::symlink("example")),
            file("usr/share/docs/foo", TestFile::new("x")),
            file("usr/share/doc/README.Debian", TestFile::new("x")).udeb(),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![
                ("usr/share/doc/bar".to_string(), TestFile::new("x")),
                (
                    "usr/share/doc/README.Debian".to_string(),
                    TestFile::new("x"),
                ),
            ]),
            file("usr/share/doc/baz", TestFile::new("x").hardlink()),
        ];
        BinaryPackageTester::new(FileDirectlyInUsrShareDoc, pass, fail).test_and_snapshot();
    }
}
