use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct InitDScriptNotIncludedInPackage;

declare_rule! {
    /// A `postinst` registers an `etc/init.d` script with `update-rc.d`, but
    /// the package does not ship it.
    ///
    /// Names are the `update-rc.d` calls the init-d check reads. The `remove`
    /// action, comments, and `if [ -x …update-rc.d` lines are ignored. A name
    /// is shipped when `etc/init.d/<name>` is a file, a directory, or a link
    /// that resolves inside the package. A link to `/lib/init/upstart-job`
    /// counts too. The `initscripts` and `sysvinit` packages are skipped. Extra
    /// is the path, in name order. There is no Location.
    InitDScriptNotIncludedInPackage,
    code = "LN0793",
    tag = "init.d-script-not-included-in-package",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptNotIncludedInPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for path in missing(ctx) {
            ctx.diagnostic(Diagnostic::error(path));
        }
    }
}

fn missing(ctx: &mut BinaryPackageContext<'_>) -> Vec<String> {
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
    let Some(data) = ctx.data_files() else {
        return Vec::new();
    };
    names
        .into_iter()
        .filter(|name| !init_script::init_d_script_shipped(data, name))
        .map(|name| format!("etc/init.d/{name}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(postinst: &str, files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(
            files
                .into_iter()
                .map(|(path, file)| (path.to_string(), file))
                .collect::<Vec<_>>(),
        )
        .control_file("postinst", TestFile::new(postinst))
    }

    #[test]
    fn test() {
        let pass = vec![
            case(
                "#!/bin/sh\nupdate-rc.d foo defaults\n",
                vec![("etc/init.d/foo", TestFile::new("#!/bin/sh\n"))],
            ),
            case(
                "update-rc.d bar defaults\n",
                vec![("etc/init.d/bar", TestFile::symlink("/lib/init/upstart-job"))],
            ),
            case(
                "update-rc.d baz defaults\n",
                vec![
                    ("etc/init.d/baz", TestFile::symlink("foo")),
                    ("etc/init.d/foo", TestFile::new("#!/bin/sh\n")),
                ],
            ),
            case("update-rc.d gone remove\n", vec![]),
            case("# update-rc.d hidden defaults\n", vec![]),
            case(
                "if [ -x /usr/sbin/update-rc.d ]; then update-rc.d excluded defaults; fi\n",
                vec![],
            ),
            case("update-rc.d missing defaults\n", vec![]).package("initscripts"),
            case("update-rc.d missing defaults\n", vec![]).package("sysvinit"),
        ];
        let fail = vec![
            case("update-rc.d foo defaults\n", vec![]),
            case(
                "update-rc.d dangling defaults\n",
                vec![("etc/init.d/dangling", TestFile::symlink("nowhere"))],
            ),
            case(
                "update-rc.d escaping defaults\n",
                vec![("etc/init.d/escaping", TestFile::symlink("../../outside"))],
            ),
            case("update-rc.d udeb-script defaults\n", vec![]).udeb(),
        ];
        BinaryPackageTester::new(InitDScriptNotIncludedInPackage, pass, fail).test_and_snapshot();
    }
}
