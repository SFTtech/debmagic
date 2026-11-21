use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct UpstartJobInEtcInitDNotRegisteredViaUpdateRcD;

declare_rule! {
    /// A direct child of `etc/init.d` is a symlink to `/lib/init/upstart-job`
    /// and no non-`remove` `update-rc.d` call in `postinst` registers it.
    ///
    /// Upstart systems ship init scripts as such links; on a non-upstart
    /// system the script then never runs, so this is usually a bug. The
    /// registered names are read the way the init-d Check reads `postinst`
    /// (comments and the `if [ -x update-rc.d ]` guard are ignored). The
    /// names `README`, `skeleton`, `rc` and `rcS` are skipped, as lintian
    /// skips them, and `initscripts` and `sysvinit` are skipped. Extra is
    /// empty; the link's path is the Location. Only symlinks are reported;
    /// a regular file of that name is `script-in-etc-init.d-not-registered-via-update-rc.d`.
    UpstartJobInEtcInitDNotRegisteredViaUpdateRcD,
    code = "LN1101",
    tag = "upstart-job-in-etc-init.d-not-registered-via-update-rc.d",
    default_selected = true,
    experimental = false,
}

const SPECIAL: [&str; 4] = ["README", "skeleton", "rc", "rcS"];

const UPSTART_JOB: &str = "/lib/init/upstart-job";

impl BinaryPackageRule for UpstartJobInEtcInitDNotRegisteredViaUpdateRcD {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        if matches!(package.as_str(), "initscripts" | "sysvinit") {
            return;
        }
        let registered = ctx
            .control_files()
            .map(init_script::registered_names)
            .unwrap_or_default();
        let Some(data) = ctx.data_files() else {
            return;
        };
        let unregistered = upstart_jobs(data, &registered);
        for path in unregistered {
            ctx.diagnostic(Diagnostic::warn("").with_location(at(&path)));
        }
    }
}

fn upstart_jobs(data: &FileTree, registered: &HashSet<String>) -> Vec<PathBuf> {
    data.entries()
        .iter()
        .filter(|entry| entry.path().parent() == Some(Path::new("etc/init.d")))
        .filter(|entry| {
            entry.kind() == EntryKind::Symlink
                && entry.link_target() == Some(Path::new(UPSTART_JOB))
        })
        .filter(|entry| {
            let name = entry.path().file_name().and_then(|name| name.to_str());
            !name.is_some_and(|name| SPECIAL.contains(&name) || registered.contains(name))
        })
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

fn at(path: &Path) -> Location {
    Location {
        path: path.to_path_buf(),
        line: None,
        column: None,
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(postinst: Option<&str>, files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        let case = BinaryPackageCase::from(
            files
                .into_iter()
                .map(|(path, file)| (path.to_string(), file))
                .collect::<Vec<_>>(),
        );
        match postinst {
            Some(text) => case.control_file("postinst", TestFile::new(text)),
            None => case,
        }
    }

    fn upstart(path: &str) -> (&str, TestFile) {
        (path, TestFile::symlink(UPSTART_JOB))
    }

    #[test]
    fn binary() {
        let file = || TestFile::new("#!/bin/sh\n");
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Registered by `update-rc.d` in postinst.
            case(
                Some("update-rc.d foo defaults\n"),
                vec![upstart("etc/init.d/foo")],
            ),
            // A regular script is lintian's other Tag.
            case(None, vec![("etc/init.d/foo", file())]),
            // Not a direct child of `etc/init.d`.
            case(
                None,
                vec![("etc/init.d/sub/foo", TestFile::symlink(UPSTART_JOB))],
            ),
            case(
                None,
                vec![("etc/default/foo", TestFile::symlink(UPSTART_JOB))],
            ),
            // Special names are skipped.
            case(
                None,
                vec![
                    upstart("etc/init.d/README"),
                    upstart("etc/init.d/skeleton"),
                    upstart("etc/init.d/rc"),
                    upstart("etc/init.d/rcS"),
                ],
            ),
            // A symlink to something else is not an upstart job.
            case(
                None,
                vec![("etc/init.d/foo", TestFile::symlink("/etc/init.d/bar"))],
            ),
            // `initscripts` and `sysvinit` are never checked.
            case(None, vec![upstart("etc/init.d/foo")]).package("initscripts"),
            case(None, vec![upstart("etc/init.d/foo")]).package("sysvinit"),
        ];
        let fail = vec![
            case(None, vec![upstart("etc/init.d/foo")]),
            // Only `update-rc.d ... remove` mentions it.
            case(
                Some("update-rc.d foo remove\n"),
                vec![upstart("etc/init.d/foo")],
            ),
            // A commented-out registration does not count.
            case(
                Some("# update-rc.d foo defaults\n"),
                vec![upstart("etc/init.d/foo")],
            ),
            // Only the registered name is exempt.
            case(
                Some("update-rc.d foo defaults\n"),
                vec![upstart("etc/init.d/foo"), upstart("etc/init.d/bar")],
            ),
            // A name that is not UTF-8 is never registered.
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"etc/init.d/bokm\xe5l".to_vec()),
                TestFile::symlink(UPSTART_JOB),
            )]),
            // Udebs are checked too.
            case(None, vec![upstart("etc/init.d/foo")]).udeb(),
        ];
        BinaryPackageTester::new(UpstartJobInEtcInitDNotRegisteredViaUpdateRcD, pass, fail)
            .test_and_snapshot();
    }
}
