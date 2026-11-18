use std::collections::HashSet;
use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct InitDScriptHasDuplicateLsbKeyword;

declare_rule! {
    /// An `etc/init.d` script sets the same keyword twice in the first LSB
    /// section.
    ///
    /// Keywords match case-insensitively. Each repeat after the first is its
    /// own finding. Extra is the lowercased keyword. The Location is the
    /// script and the repeated line. The same scripts are skipped as for the
    /// required-option Tag.
    InitDScriptHasDuplicateLsbKeyword,
    code = "LN0742",
    tag = "init.d-script-has-duplicate-lsb-keyword",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptHasDuplicateLsbKeyword {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for script in scripts(ctx) {
            let mut seen = HashSet::new();
            for (line, keyword) in &script.section.keywords {
                if !seen.insert(keyword.as_str()) {
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
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n# Description: b\n### END INIT INFO\n",
            ),
            script(
                "indented",
                "#!/bin/sh\n### BEGIN INIT INFO\n#  Provides: a\n# Provides: b\n### END INIT INFO\n",
            ),
            script(
                "second-section",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n### END INIT INFO\n### BEGIN INIT INFO\n# Provides: b\n### END INIT INFO\n",
            ),
            script("outside", "#!/bin/sh\n# Provides: a\n# Provides: b\n"),
            script(
                "skeleton",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n# Provides: b\n### END INIT INFO\n",
            ),
            script(
                "pkg",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n# Provides: b\n### END INIT INFO\n",
            )
            .package("initscripts"),
        ];
        let fail = vec![
            script(
                "short",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Short-Description: a\n# Short-Description: b\n### END INIT INFO\n",
            ),
            script(
                "case",
                "#!/bin/sh\n### BEGIN INIT INFO\n# X-Foo: a\n# x-foo: b\n# X-FOO: c\n### END INIT INFO\n",
            ),
            script(
                "unterminated",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n# Provides: b\n\n",
            ),
            script(
                "udeb-script",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n# Provides: b\n### END INIT INFO\n",
            )
            .udeb(),
        ];
        BinaryPackageTester::new(InitDScriptHasDuplicateLsbKeyword, pass, fail).test_and_snapshot();
    }
}
