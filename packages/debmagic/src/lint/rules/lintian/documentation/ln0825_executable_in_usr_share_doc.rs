use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, FileTree};
use crate::lint::interpreter::Hashbang;
use crate::lint::rule::BinaryPackageRule;

pub struct ExecutableInUsrShareDoc;

declare_rule! {
    /// A regular file or hardlink under `usr/share/doc/` has an execute bit
    /// and is not a script.
    ///
    /// Walks Data files. Files under an `examples/` directory (directly under
    /// the doc root or one package level down) are left out. Scripts, which
    /// start with `#!` (lincity `#!#!#!` excluded), are another Tag and are
    /// left out too. Unreadable content is not reported. Extra is the
    /// permission bits in four-digit octal; the path is the Location. Udebs
    /// are checked too.
    ExecutableInUsrShareDoc,
    code = "LN0825",
    tag = "executable-in-usr-share-doc",
    default_selected = true,
    experimental = false,
}

static DOC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/share/doc/\S").expect("valid regex"));
static EXAMPLES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/share/doc/(?:[^/]+/)?examples/").expect("valid regex"));

impl BinaryPackageRule for ExecutableInUsrShareDoc {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<(PathBuf, String)> = files
            .entries()
            .iter()
            .filter(|entry| is_candidate(entry) && is_not_script(files, entry))
            .map(|entry| (entry.path().to_path_buf(), format!("{:04o}", entry.mode())))
            .collect();
        for (path, message) in hits {
            ctx.diagnostic(Diagnostic::error(message).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// An executable file under `usr/share/doc/`, outside `examples/`.
fn is_candidate(entry: &Entry) -> bool {
    let Some(path) = entry.path().to_str() else {
        return false;
    };
    entry.is_executable_file() && DOC.is_match(path) && !EXAMPLES.is_match(path)
}

/// No `#!` line. Unreadable content is not reported.
fn is_not_script(files: &FileTree, entry: &Entry) -> bool {
    files
        .read(entry)
        .is_ok_and(|bytes| Hashbang::read(&bytes).is_none())
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn data(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, file)])
    }

    fn exec(content: &str) -> TestFile {
        TestFile::new(content).mode(0o755)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            data(
                "usr/share/doc/pkg/README",
                TestFile::new("text").mode(0o644),
            ),
            // Examples are left out, at both depths.
            data("usr/share/doc/pkg/examples/tool", exec("not a script\n")),
            data("usr/share/doc/examples/tool", exec("not a script\n")),
            data(
                "usr/share/doc/pkg/examples/sub/tool",
                exec("not a script\n"),
            ),
            // Scripts are another Tag.
            data("usr/share/doc/pkg/helper", exec("#!/bin/sh\necho hi\n")),
            // Outside usr/share/doc/.
            data("usr/bin/tool", exec("not a script\n")),
            data("usr/share/docs/tool", exec("not a script\n")),
            // Directories and symlinks are not files.
            data("usr/share/doc/pkg/dir", TestFile::directory().mode(0o755)),
            data(
                "usr/share/doc/pkg/link",
                TestFile::symlink("README").mode(0o777),
            ),
        ];
        let fail = vec![
            data("usr/share/doc/pkg/tool", exec("not a script\n")),
            data(
                "usr/share/doc/pkg/tool",
                TestFile::new("not a script\n").mode(0o750),
            ),
            data(
                "usr/share/doc/pkg/setuid",
                TestFile::new("not a script\n").mode(0o4755),
            ),
            // Lincity magic is not a script.
            data("usr/share/doc/pkg/lincity", exec("#!#!#!\n")),
            // Only the exact examples directory name is exempt.
            data("usr/share/doc/examples-x/tool", exec("not a script\n")),
            // Directly in the doc root.
            data("usr/share/doc/tool", exec("not a script\n")),
            data("usr/share/doc/pkg/tool", exec("not a script\n")).udeb(),
        ];
        BinaryPackageTester::new(ExecutableInUsrShareDoc, pass, fail).test_and_snapshot();
    }
}
