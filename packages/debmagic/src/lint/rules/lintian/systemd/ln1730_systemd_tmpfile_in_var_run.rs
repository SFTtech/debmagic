use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::Entry;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct SystemdTmpfileInVarRun;

declare_rule! {
    /// A tmpfiles.d snippet creates a directory under `/var/run/`.
    ///
    /// Walks Data files. An entry whose path is `usr/lib/tmpfiles.d/*.conf`
    /// is reported when its text has a line that starts with `d`, whitespace,
    /// and `/var/run/`. Content that is not valid UTF-8 is read as empty, as
    /// lintian does, so it is not reported. Symlinks have no content to read
    /// and are skipped. Extra is empty; the Location is the file. Udebs are
    /// checked too.
    SystemdTmpfileInVarRun,
    code = "LN1730",
    tag = "systemd-tmpfile-in-var-run",
    default_selected = false,
    experimental = false,
}

static CONF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/lib/tmpfiles\.d/.*\.conf$").expect("valid regex"));
static VAR_RUN_DIRECTORY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^d\s+/var/run/").expect("valid regex"));

impl BinaryPackageRule for SystemdTmpfileInVarRun {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| is_var_run_tmpfile(files, entry))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in hits {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

fn is_var_run_tmpfile(files: &crate::lint::file_tree::FileTree, entry: &Entry) -> bool {
    let Some(path) = entry.path().to_str() else {
        return false;
    };
    if !CONF.is_match(path) {
        return false;
    }
    files.read(entry).is_ok_and(|bytes| {
        std::str::from_utf8(&bytes).is_ok_and(|text| VAR_RUN_DIRECTORY.is_match(text))
    })
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn conf(path: &str, text: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(text))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // The modern location.
            conf(
                "usr/lib/tmpfiles.d/test.conf",
                "d /run/test 0755 root root -\n",
            ),
            // Not under usr/lib/tmpfiles.d.
            conf(
                "etc/tmpfiles.d/test.conf",
                "d /var/run/test 0755 root root -\n",
            ),
            // Not a .conf file.
            conf(
                "usr/lib/tmpfiles.d/test.txt",
                "d /var/run/test 0755 root root -\n",
            ),
            // Another type letter, or the path only inside a word.
            conf(
                "usr/lib/tmpfiles.d/test.conf",
                "f /var/run/test 0644 root root -\n",
            ),
            conf("usr/lib/tmpfiles.d/test.conf", "# d /var/run/test\n"),
            conf("usr/lib/tmpfiles.d/test.conf", "x d /var/run/test\n"),
            // A lookalike prefix is not /var/run/.
            conf("usr/lib/tmpfiles.d/test.conf", "d /var/runner/test\n"),
            // A symlink has no content.
            BinaryPackageCase::from(vec![(
                "usr/lib/tmpfiles.d/test.conf",
                TestFile::symlink("/etc/tmpfiles.d/test.conf"),
            )]),
        ];
        let fail = vec![
            conf(
                "usr/lib/tmpfiles.d/test.conf",
                "d /var/run/test 0755 root root -\n",
            ),
            // Any line of the file counts.
            conf(
                "usr/lib/tmpfiles.d/test.conf",
                "# header\nf /run/x\nd   /var/run/test 0755 root root -\n",
            ),
            // Udebs are checked too.
            conf("usr/lib/tmpfiles.d/test.conf", "d /var/run/test\n").udeb(),
        ];
        BinaryPackageTester::new(SystemdTmpfileInVarRun, pass, fail).test_and_snapshot();
    }
}
