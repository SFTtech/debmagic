use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct BadPermissionsForEtcEmacsScript;

declare_rule! {
    /// A file below `etc/emacs*/` is not mode 0644.
    ///
    /// Walks Data files regular files and hardlinks whose path matches
    /// `etc/emacs.*/<non-space>`, so a file directly in `etc/emacs*` is not
    /// checked. Any mode other than `0644`, the executable bits included, is
    /// reported with the extra `<mode> != 0644`. The path is the Location.
    /// Udebs are checked too.
    BadPermissionsForEtcEmacsScript,
    code = "LN0837",
    tag = "bad-permissions-for-etc-emacs-script",
    default_selected = true,
    experimental = false,
}

const WIDELY_READABLE: u32 = 0o644;

static EMACS_SCRIPT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^etc/emacs.*/\S").expect("valid regex"));

impl BinaryPackageRule for BadPermissionsForEtcEmacsScript {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| {
                matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
                    && entry
                        .path()
                        .to_str()
                        .is_some_and(|path| EMACS_SCRIPT.is_match(path))
                    && entry.mode() != WIDELY_READABLE
            })
            .map(|entry| (entry.path().to_path_buf(), entry.mode()))
            .collect();
        for (path, mode) in hits {
            ctx.diagnostic(
                Diagnostic::error(format!("{mode:04o} != {WIDELY_READABLE:04o}")).with_location(
                    Location {
                        path,
                        line: None,
                        column: None,
                    },
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn tree(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files)
    }

    fn file(mode: u32) -> TestFile {
        TestFile::new("(setq x 1)\n").mode(mode)
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(vec![]),
            tree(vec![("etc/emacs.d/script", file(0o644))]),
            tree(vec![("etc/emacs/site-start.d/50x.el", file(0o644))]),
            // Not a file below etc/emacs*/: other paths.
            tree(vec![("etc/emacs-config", file(0o755))]),
            tree(vec![("etc/emacs.d/", TestFile::directory())]),
            tree(vec![("etc/emacs.d/sub", TestFile::directory())]),
            tree(vec![("etc/emacs.d/link", TestFile::symlink("script"))]),
            tree(vec![("usr/share/emacs/site-lisp/x.el", file(0o755))]),
            tree(vec![("etc/other/script", file(0o755))]),
        ];
        let fail = vec![
            tree(vec![("etc/emacs.d/script", file(0o755))]),
            tree(vec![("etc/emacs/site-start.d/50x.el", file(0o600))]),
            tree(vec![("etc/emacs.d/sub/script", file(0o664))]),
            tree(vec![("etc/emacs.d/script", file(0o755).hardlink())]),
            tree(vec![("etc/emacs.d/script", file(0o755))]).udeb(),
        ];
        BinaryPackageTester::new(BadPermissionsForEtcEmacsScript, pass, fail).test_and_snapshot();
    }
}
