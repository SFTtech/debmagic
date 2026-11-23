use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, FileTree};
use crate::lint::interpreter::Hashbang;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct BashCompletionWithHashbang;

declare_rule! {
    /// A bash completion script starts with a `#!` line.
    ///
    /// Every Data files entry under `usr/share/bash-completion/completions/`
    /// whose content starts with a `#!` line with a non-empty text after it is
    /// reported. Completion scripts are sourced, not executed, so the line
    /// should go. Extra is the trimmed hashbang text, such as `/bin/sh`; the
    /// Location is line 1 of the file. Only files and hardlinks have content,
    /// so directories and symlinks are skipped. Unreadable content is not
    /// reported. Lincity-style `#!#!#!` content is not a hashbang. Udebs are
    /// checked too.
    BashCompletionWithHashbang,
    code = "LN1721",
    tag = "bash-completion-with-hashbang",
    default_selected = false,
    experimental = false,
}

const COMPLETIONS: &str = "usr/share/bash-completion/completions/";

impl BinaryPackageRule for BashCompletionWithHashbang {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<(std::path::PathBuf, String)> = files
            .entries()
            .iter()
            .filter(|entry| is_completion(entry))
            .filter_map(|entry| {
                let hashbang = hashbang_text(files, entry)?;
                Some((entry.path().to_path_buf(), hashbang))
            })
            .collect();
        for (path, extra) in hits {
            ctx.diagnostic(
                Diagnostic::warn(extra)
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path,
                        line: Some(1),
                        column: None,
                    }),
            );
        }
    }
}

/// A file or hardlink under the completions directory.
fn is_completion(entry: &Entry) -> bool {
    entry.is_file()
        && entry
            .path()
            .to_str()
            .is_some_and(|path| path.starts_with(COMPLETIONS))
}

/// The non-empty trimmed hashbang text, when the file starts with `#!`.
fn hashbang_text(files: &FileTree, entry: &Entry) -> Option<String> {
    let bytes = files.read(entry).ok()?;
    let hashbang = Hashbang::read(&bytes)?;
    let line = hashbang.line();
    if line.is_empty() {
        return None;
    }
    Some(String::from_utf8_lossy(line).into_owned())
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn text(path: &str, content: &str) -> (String, TestFile) {
        (path.to_string(), TestFile::new(content))
    }

    fn case(entries: Vec<(String, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(entries)
    }

    #[test]
    fn binary() {
        let pass = vec![
            case(vec![]),
            // Sourced completion without a hashbang.
            case(vec![text(
                "usr/share/bash-completion/completions/foo",
                "# completion\ncomplete -F _foo foo\n",
            )]),
            // A bare `#!` has no interpreter text.
            case(vec![text(
                "usr/share/bash-completion/completions/foo",
                "#!\n",
            )]),
            // Only the completions directory is checked.
            case(vec![text("usr/share/foo/bar", "#!/bin/sh\n")]),
            // Lincity-style data is not a hashbang.
            case(vec![text(
                "usr/share/bash-completion/completions/foo",
                "#!#!#!\n",
            )]),
            // Directories and symlinks have no content to read.
            case(vec![(
                "usr/share/bash-completion/completions/sub".to_string(),
                TestFile::directory(),
            )]),
            case(vec![(
                "usr/share/bash-completion/completions/foo".to_string(),
                TestFile::symlink("bar"),
            )]),
        ];
        let fail = vec![
            case(vec![text(
                "usr/share/bash-completion/completions/foo",
                "#!/bin/sh\ncomplete -F _foo foo\n",
            )]),
            // Interpreter options are kept in the extra text, trimmed.
            case(vec![text(
                "usr/share/bash-completion/completions/foo",
                "#!  /usr/bin/env bash -e  \n",
            )]),
            // Subdirectories of the completions directory are checked too.
            case(vec![text(
                "usr/share/bash-completion/completions/sub/foo",
                "#!/bin/sh\n",
            )]),
        ];
        BinaryPackageTester::new(BashCompletionWithHashbang, pass, fail).test_and_snapshot();
    }
}
