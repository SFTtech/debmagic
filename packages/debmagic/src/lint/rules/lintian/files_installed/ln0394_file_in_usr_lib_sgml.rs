use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::BinaryPackageRule;

pub struct FileInUsrLibSgml;

declare_rule! {
    /// A file is installed under `usr/lib/sgml/`.
    ///
    /// Any Data files entry whose path matches `usr/lib/sgml/` plus one
    /// non-whitespace character. The directory `usr/lib/sgml` itself is not
    /// flagged. Extra is empty. The Location is the path. Udebs are checked.
    FileInUsrLibSgml,
    code = "LN0394",
    tag = "file-in-usr-lib-sgml",
    default_selected = true,
    experimental = false,
}

static SGML: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^usr/lib/sgml/\S").expect("regex"));

impl BinaryPackageRule for FileInUsrLibSgml {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for path in matching(files, &SGML) {
            ctx.diagnostic(Diagnostic::warn("").with_location(at(path)));
        }
    }
}

pub(super) fn matching(files: &FileTree, pattern: &Regex) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = files
        .entries()
        .iter()
        .filter_map(|entry| {
            let name = entry.path().to_str()?;
            pattern.is_match(name).then(|| entry.path().to_path_buf())
        })
        .collect();
    paths.sort();
    paths
}

pub(super) fn at(path: PathBuf) -> Location {
    Location {
        path,
        line: None,
        column: None,
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn file(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new("catalog"))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            file("usr/lib/sgml"),
            file("usr/share/sgml/package"),
            file("usr/lib/sgml2/package"),
            file("usr/lib/sgml/ package"),
        ];
        let fail = vec![
            file("usr/lib/sgml/package"),
            file("usr/lib/sgml/bar"),
            BinaryPackageCase::from(vec![(
                "usr/lib/sgml/dir".to_string(),
                TestFile::directory(),
            )]),
            file("usr/lib/sgml/nested/file").udeb(),
        ];
        BinaryPackageTester::new(FileInUsrLibSgml, pass, fail).test_and_snapshot();
    }
}
