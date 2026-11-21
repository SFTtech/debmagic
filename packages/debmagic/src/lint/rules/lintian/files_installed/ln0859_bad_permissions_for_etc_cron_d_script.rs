use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct BadPermissionsForEtcCronDScript;

declare_rule! {
    /// An entry below `etc/cron.d/` is not mode 0644.
    ///
    /// Walks every Data files entry (files, directories, symlinks and
    /// hardlinks, as lintian visits every installed item) whose path matches
    /// `etc/cron.d/<non-dot>`, so a `.placeholder` file is not checked. Any mode
    /// other than `0644` is reported with the extra `<mode> != 0644`, the
    /// executable bits included. The path is the Location.
    BadPermissionsForEtcCronDScript,
    code = "LN0859",
    tag = "bad-permissions-for-etc-cron.d-script",
    default_selected = true,
    experimental = false,
}

const READ_WRITE: u32 = 0o644;

static CRON_D_SCRIPT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^etc/cron\.d/[^.]").expect("valid regex"));

impl BinaryPackageRule for BadPermissionsForEtcCronDScript {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| {
                entry
                    .path()
                    .to_str()
                    .is_some_and(|path| CRON_D_SCRIPT.is_match(path))
                    && entry.mode() != READ_WRITE
            })
            .map(|entry| (entry.path().to_path_buf(), entry.mode()))
            .collect();
        for (path, mode) in hits {
            ctx.diagnostic(
                Diagnostic::error(format!("{mode:04o} != {READ_WRITE:04o}")).with_location(
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

    fn script(mode: u32) -> TestFile {
        TestFile::new("* * * * * root true\n").mode(mode)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![("etc/cron.d/foo", script(0o644))]),
            // Dot-files (cron ships .placeholder) are not checked.
            BinaryPackageCase::from(vec![("etc/cron.d/.placeholder", script(0o755))]),
            // Only etc/cron.d/ counts.
            BinaryPackageCase::from(vec![("etc/cron.daily/foo", script(0o755))]),
            BinaryPackageCase::from(vec![("etc/cron.d.foo", script(0o755))]),
            BinaryPackageCase::from(vec![("usr/etc/cron.d/foo", script(0o755))]),
            // The directory itself is not an entry below it.
            BinaryPackageCase::from(vec![("etc/cron.d/", TestFile::directory().mode(0o755))]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("etc/cron.d/foo", script(0o755))]),
            BinaryPackageCase::from(vec![("etc/cron.d/foo", script(0o600))]),
            BinaryPackageCase::from(vec![("etc/cron.d/foo", script(0o664))]),
            BinaryPackageCase::from(vec![("etc/cron.d/foo", script(0o755).hardlink())]),
            // Directories are installed items too.
            BinaryPackageCase::from(vec![("etc/cron.d/sub", TestFile::directory().mode(0o775))]),
            // A udeb is still a Binary package.
            BinaryPackageCase::from(vec![("etc/cron.d/foo", script(0o755))]).udeb(),
        ];
        BinaryPackageTester::new(BadPermissionsForEtcCronDScript, pass, fail).test_and_snapshot();
    }
}
