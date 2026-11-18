use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct InitDScriptMissingDependencyOnLocalFs;

declare_rule! {
    /// An `etc/init.d` script uses `/var/`, but does not depend on
    /// `$local_fs` for a runlevel it declares.
    ///
    /// A body line counts when the text before its first `#` contains `/var/`.
    /// Lines of the first LSB section do not count. The start check needs a
    /// non-empty `Default-Start` and no `$local_fs`, `$remote_fs`, or `$all` in
    /// `Required-Start`. The stop check needs a non-empty `Default-Stop` and no
    /// `$local_fs`, `$remote_fs`, `$all`, `umountfs`, or `umountnfs` in
    /// `Required-Stop`. Each failing keyword is its own finding. Extra is the
    /// keyword. The script path is the Location. The same scripts are skipped
    /// as for the required-option Tag.
    InitDScriptMissingDependencyOnLocalFs,
    code = "LN0791",
    tag = "init.d-script-missing-dependency-on-local_fs",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptMissingDependencyOnLocalFs {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for script in scripts(ctx) {
            if !uses_var(&script.text) {
                continue;
            }
            if !script.lsb.default_start.is_empty()
                && !script.lsb.required_start.iter().any(|facility| {
                    matches!(facility.as_str(), "$local_fs" | "$remote_fs" | "$all")
                })
            {
                ctx.diagnostic(Diagnostic::error("required-start").with_location(at(&script.path)));
            }
            if !script.lsb.default_stop.is_empty()
                && !script.lsb.required_stop.iter().any(|facility| {
                    matches!(
                        facility.as_str(),
                        "$local_fs" | "$remote_fs" | "$all" | "umountfs" | "umountnfs"
                    )
                })
            {
                ctx.diagnostic(Diagnostic::error("required-stop").with_location(at(&script.path)));
            }
        }
    }
}

/// Whether a body line before its first `#` contains `/var/`.
fn uses_var(text: &str) -> bool {
    init_script::body_lines(text).iter().any(|(_, line)| {
        line.split('#')
            .next()
            .is_some_and(|code| code.contains("/var/"))
    })
}

fn scripts(ctx: &mut BinaryPackageContext<'_>) -> Vec<init_script::InitDScript> {
    let package = ctx.package_name().to_string();
    let registered = {
        let Some(control) = ctx.control_files() else {
            return Vec::new();
        };
        init_script::registered_names(control)
    };
    let Some(data) = ctx.data_files() else {
        return Vec::new();
    };
    init_script::init_d_scripts(data, &registered, &package)
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

    fn script(name: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(format!("etc/init.d/{name}"), TestFile::new(body))])
    }

    fn lsb(
        start: &str,
        required_start: &str,
        stop: &str,
        required_stop: &str,
        body: &str,
    ) -> String {
        format!(
            "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n# Required-Start: {required_start}\n# Required-Stop: {required_stop}\n# Default-Start: {start}\n# Default-Stop: {stop}\n### END INIT INFO\n{body}\n"
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            script(
                "local",
                &lsb("2 3 4 5", "$local_fs", "0 1 6", "$local_fs", "ls /var/x"),
            ),
            script(
                "remote",
                &lsb("2 3 4 5", "$remote_fs", "0 1 6", "umountfs", "ls /var/x"),
            ),
            script(
                "all",
                &lsb("2 3 4 5", "$all", "0 1 6", "umountnfs", "ls /var/x"),
            ),
            script(
                "no-var",
                &lsb("2 3 4 5", "$network", "0 1 6", "$network", "ls /usr"),
            ),
            script(
                "comment",
                &lsb("2 3 4 5", "$network", "0 1 6", "$network", "# ls /var/x"),
            ),
            script(
                "no-start",
                &lsb("", "$network", "0 1 6", "$local_fs", "ls /var/x"),
            ),
            script(
                "no-stop",
                &lsb("2 3 4 5", "$local_fs", "", "$network", "ls /var/x"),
            ),
            script(
                "unterminated",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Default-Start: 2\n# Required-Start: $network\nls /var/x\n",
            ),
            script(
                "pkg",
                &lsb("2 3 4 5", "$network", "0 1 6", "$network", "ls /var/x"),
            )
            .package("initscripts"),
        ];
        let fail = vec![
            script(
                "start",
                &lsb("2 3 4 5", "$network", "0 1 6", "$local_fs", "ls /var/x"),
            ),
            script(
                "stop",
                &lsb("2 3 4 5", "$local_fs", "0 1 6", "$network", "ls /var/x"),
            ),
            script(
                "both",
                &lsb("2 3 4 5", "$network", "0 1 6", "$network", "ls /var/x"),
            ),
            script(
                "udeb-script",
                &lsb("2 3 4 5", "$network", "0 1 6", "$network", "ls /var/x"),
            )
            .udeb(),
        ];
        BinaryPackageTester::new(InitDScriptMissingDependencyOnLocalFs, pass, fail)
            .test_and_snapshot();
    }
}
