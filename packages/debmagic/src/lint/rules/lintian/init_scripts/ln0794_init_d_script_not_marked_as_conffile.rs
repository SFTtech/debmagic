use std::collections::HashSet;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::init_script;
use crate::lint::rules::lintian::conffiles::declared::declared;

pub struct InitDScriptNotMarkedAsConffile;

declare_rule! {
    /// An `etc/init.d` script registered by `postinst` is shipped as a regular
    /// file or hardlink, but is not listed in `conffiles`.
    ///
    /// The script set is the one for LN0793, and a name must be shipped by
    /// that rule's test. A symlink is never flagged, including one to the
    /// upstart-job. A `conffiles` entry matches the path with its leading `/`
    /// removed. The `initscripts` and `sysvinit` packages are skipped. Extra is
    /// the path, in name order. There is no Location.
    InitDScriptNotMarkedAsConffile,
    code = "LN0794",
    tag = "init.d-script-not-marked-as-conffile",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptNotMarkedAsConffile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for path in unmarked(ctx) {
            ctx.diagnostic(Diagnostic::warn(path));
        }
    }
}

fn unmarked(ctx: &mut BinaryPackageContext<'_>) -> Vec<String> {
    let package = ctx.package_name().to_string();
    if matches!(package.as_str(), "initscripts" | "sysvinit") {
        return Vec::new();
    }
    let mut names: Vec<String> = {
        let Some(control) = ctx.control_files() else {
            return Vec::new();
        };
        init_script::registered_names(control).into_iter().collect()
    };
    names.sort();
    let conffiles: HashSet<String> = declared(ctx).into_iter().map(|entry| entry.path).collect();
    let Some(data) = ctx.data_files() else {
        return Vec::new();
    };
    names
        .into_iter()
        .filter(|name| init_script::init_d_script_shipped(data, name))
        .filter_map(|name| {
            let path = format!("etc/init.d/{name}");
            let entry = data.get(&path)?;
            (entry.kind() != EntryKind::Symlink && !conffiles.contains(&path)).then_some(path)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(
        postinst: &str,
        conffiles: Option<&str>,
        files: Vec<(&str, TestFile)>,
    ) -> BinaryPackageCase {
        let case = BinaryPackageCase::from(
            files
                .into_iter()
                .map(|(path, file)| (path.to_string(), file))
                .collect::<Vec<_>>(),
        )
        .control_file("postinst", TestFile::new(postinst));
        match conffiles {
            Some(text) => case.control_file("conffiles", TestFile::new(text)),
            None => case,
        }
    }

    #[test]
    fn test() {
        let file = || TestFile::new("#!/bin/sh\n");
        let pass = vec![
            case(
                "update-rc.d foo defaults\n",
                Some("/etc/init.d/foo\n"),
                vec![("etc/init.d/foo", file())],
            ),
            case(
                "update-rc.d bar defaults\n",
                None,
                vec![("etc/init.d/bar", TestFile::symlink("/lib/init/upstart-job"))],
            ),
            case(
                "update-rc.d baz defaults\n",
                None,
                vec![
                    ("etc/init.d/baz", TestFile::symlink("foo")),
                    ("etc/init.d/foo", file()),
                ],
            ),
            case("update-rc.d missing defaults\n", None, vec![]),
            case("update-rc.d gone remove\n", None, vec![]),
            case(
                "update-rc.d foo defaults\n",
                None,
                vec![("etc/init.d/foo", file())],
            )
            .package("initscripts"),
        ];
        let fail = vec![
            case(
                "update-rc.d foo defaults\n",
                None,
                vec![("etc/init.d/foo", file())],
            ),
            case(
                "update-rc.d foo defaults\n",
                Some("/etc/init.d/other\n"),
                vec![("etc/init.d/foo", file())],
            ),
            case(
                "update-rc.d udeb-script defaults\n",
                None,
                vec![("etc/init.d/udeb-script", file())],
            )
            .udeb(),
        ];
        BinaryPackageTester::new(InitDScriptNotMarkedAsConffile, pass, fail).test_and_snapshot();
    }
}
