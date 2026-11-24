use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct InitDScriptHasUnknownLsbKeyword;

declare_rule! {
    /// An `etc/init.d` script sets a keyword in the first LSB section that is
    /// not a known LSB keyword and does not start with `X-`.
    ///
    /// Known keywords are `Provides`, `Required-Start`, `Required-Stop`,
    /// `Should-Start`, `Should-Stop`, `Default-Start`, `Default-Stop`,
    /// `Short-Description`, and `Description`, matched case-insensitively.
    /// Each occurrence is its own finding. Extra is the lowercased keyword.
    /// The Location is the script and the line. The same scripts are skipped
    /// as for the required-option Tag.
    InitDScriptHasUnknownLsbKeyword,
    code = "LN0744",
    tag = "init.d-script-has-unknown-lsb-keyword",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptHasUnknownLsbKeyword {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for script in scripts(ctx) {
            for (line, keyword) in &script.section.keywords {
                if !init_script::lsb_keyword_known(keyword) {
                    ctx.diagnostic(
                        Diagnostic::warn(keyword.clone()).with_location(at(&script.path, *line)),
                    );
                }
            }
        }
    }
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

fn at(path: &Path, line: u32) -> Location {
    Location {
        path: path.to_path_buf(),
        line: Some(line),
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

    #[test]
    fn test() {
        let pass = vec![
            script(
                "ok",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n# Short-Description: b\n# Description: c\n### END INIT INFO\n",
            ),
            script(
                "extension",
                "#!/bin/sh\n### BEGIN INIT INFO\n# X-Debian-Foo: a\n# x-bar: b\n### END INIT INFO\n",
            ),
            script(
                "case",
                "#!/bin/sh\n### BEGIN INIT INFO\n# SHOULD-START: a\n# default-stop: b\n### END INIT INFO\n",
            ),
            script(
                "indented",
                "#!/bin/sh\n### BEGIN INIT INFO\n#  Foo: a\n### END INIT INFO\n",
            ),
            script("outside", "#!/bin/sh\n# Foo: a\n"),
            script(
                "skeleton",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Foo: a\n### END INIT INFO\n",
            ),
            script(
                "pkg",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Foo: a\n### END INIT INFO\n",
            )
            .package("sysvinit"),
        ];
        let fail = vec![
            script(
                "foo",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n# Foo: a\n### END INIT INFO\n",
            ),
            script(
                "typo",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Short-Desc: a\n# Short-Desc: b\n### END INIT INFO\n",
            ),
            script(
                "udeb-script",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Foo: a\n### END INIT INFO\n",
            )
            .udeb(),
        ];
        BinaryPackageTester::new(InitDScriptHasUnknownLsbKeyword, pass, fail).test_and_snapshot();
    }
}
