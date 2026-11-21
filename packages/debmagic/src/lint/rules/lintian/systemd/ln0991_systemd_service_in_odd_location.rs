use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct SystemdServiceInOddLocation;

declare_rule! {
    /// A `*.service` unit is installed under `etc/systemd/system`.
    ///
    /// Any kind of entry counts, including a symbolic link (a masked unit) and
    /// a special file, but not a directory. Extra is empty. The unit path is
    /// the Location. Udebs are checked too.
    SystemdServiceInOddLocation,
    code = "LN0991",
    tag = "systemd-service-in-odd-location",
    default_selected = true,
    experimental = false,
}

static SERVICE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/systemd/system/.*\.service$").expect("valid regex"));

impl BinaryPackageRule for SystemdServiceInOddLocation {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .query()
            .path(&SERVICE)
            .into_iter()
            .filter(|entry| entry.kind() != EntryKind::Directory)
            .filter(|entry| entry.path().starts_with("etc/systemd/system"))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(at(path)));
        }
    }
}

fn at(path: PathBuf) -> Location {
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

    fn unit(path: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new(body))])
    }

    #[test]
    fn test() {
        let pass = vec![
            unit(
                "usr/lib/systemd/system/good.service",
                "[Service]\nExecStart=/bin/true\n",
            ),
            unit(
                "etc/systemd/user/user.service",
                "[Service]\nExecStart=/bin/true\n",
            ),
            unit("etc/systemd/system/notes.conf", "x\n"),
            BinaryPackageCase::from(vec![(
                "etc/systemd/system/dir.service".to_string(),
                TestFile::directory(),
            )]),
        ];
        let fail = vec![
            unit(
                "etc/systemd/system/local.service",
                "[Service]\nExecStart=/bin/true\n",
            ),
            BinaryPackageCase::from(vec![(
                "etc/systemd/system/masked.service".to_string(),
                TestFile::symlink("/dev/null"),
            )]),
            unit(
                "etc/systemd/system/udeb.service",
                "[Service]\nExecStart=/bin/true\n",
            )
            .udeb(),
        ];
        BinaryPackageTester::new(SystemdServiceInOddLocation, pass, fail).test_and_snapshot();
    }
}
