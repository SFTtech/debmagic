use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct ScriptInEtcInitDNotRegisteredViaUpdateRcD;

declare_rule! {
    /// A direct child of `etc/init.d` is not registered with `update-rc.d`
    /// in `postinst`.
    ///
    /// Every child is checked, except names registered by a non-`remove`
    /// `update-rc.d` call in `postinst` (comments and the `if [ -x
    /// update-rc.d ]` guard are ignored, as the init-d check reads them). A
    /// non-directory named `README`, `skeleton`, `rc`, or `rcS` is skipped. A
    /// symlink to `/lib/init/upstart-job` is skipped too, since lintian
    /// reports it under its own upstart Tag. `initscripts` and `sysvinit` are
    /// skipped. The extra is empty and the path is the Location. Directories
    /// are reported like files.
    ScriptInEtcInitDNotRegisteredViaUpdateRcD,
    code = "LN0948",
    tag = "script-in-etc-init.d-not-registered-via-update-rc.d",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ScriptInEtcInitDNotRegisteredViaUpdateRcD {
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
        let unregistered = unregistered_scripts(data, &registered);
        for path in unregistered {
            ctx.diagnostic(Diagnostic::warn("").with_location(at(&path)));
        }
    }
}

const SPECIAL: [&str; 4] = ["README", "skeleton", "rc", "rcS"];

fn unregistered_scripts(data: &FileTree, registered: &HashSet<String>) -> Vec<PathBuf> {
    data.entries()
        .iter()
        .filter(|entry| entry.path().parent() == Some(Path::new("etc/init.d")))
        .filter(|entry| {
            let Some(name) = entry.path().file_name() else {
                return false;
            };
            let text = name.to_str();
            let special = text.is_some_and(|name| SPECIAL.contains(&name));
            let is_registered = text.is_some_and(|name| registered.contains(name));
            let upstart = entry.kind() == EntryKind::Symlink
                && entry.link_target() == Some(Path::new("/lib/init/upstart-job"));
            !(entry.kind() != EntryKind::Directory && special) && !is_registered && !upstart
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

    #[test]
    fn binary() {
        let file = || TestFile::new("#!/bin/sh\n");
        let pass = vec![
            case(
                Some("update-rc.d foo defaults\n"),
                vec![("etc/init.d/foo", file())],
            ),
            case(
                Some("update-rc.d foo defaults >/dev/null\nupdate-rc.d bar defaults\n"),
                vec![("etc/init.d/foo", file()), ("etc/init.d/bar", file())],
            ),
            case(
                None,
                vec![("etc/init.d/bar", TestFile::symlink("/lib/init/upstart-job"))],
            ),
            case(
                None,
                vec![
                    ("etc/init.d/README", file()),
                    ("etc/init.d/skeleton", file()),
                    ("etc/init.d/rc", file()),
                    ("etc/init.d/rcS", file()),
                ],
            ),
            case(None, vec![("etc/default/foo", file())]),
            case(None, vec![("etc/init.d/foo", file())]).package("initscripts"),
            case(None, vec![("etc/init.d/foo", file())]).package("sysvinit"),
        ];
        let fail = vec![
            case(None, vec![("etc/init.d/foo", file())]),
            case(
                Some("update-rc.d foo defaults\n"),
                vec![("etc/init.d/foo", file()), ("etc/init.d/bar", file())],
            ),
            case(
                Some("# update-rc.d foo defaults\n"),
                vec![("etc/init.d/foo", file())],
            ),
            case(
                Some("update-rc.d foo remove\n"),
                vec![("etc/init.d/foo", file())],
            ),
            case(
                None,
                vec![
                    ("etc/init.d/foo", file()),
                    ("etc/init.d/baz", TestFile::symlink("foo")),
                ],
            ),
            case(None, vec![("etc/init.d/README", TestFile::directory())]),
            case(None, vec![("etc/init.d/foo", file())]).udeb(),
        ];
        BinaryPackageTester::new(ScriptInEtcInitDNotRegisteredViaUpdateRcD, pass, fail)
            .test_and_snapshot();
    }
}
