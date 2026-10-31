use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::script::{Continuations, ScriptLine, maintainer_script_lines};

pub struct MaintainerScriptCallsInitScriptDirectly;

declare_rule! {
    /// A maintainer script runs an `/etc/init.d` script directly.
    ///
    /// A direct call is a code line that starts with `/etc/init.d/<name> <action>`,
    /// with optional quotes around the action. `invoke-rc.d` at the start of any
    /// code line in that same script suppresses the tag. One finding per script,
    /// on the last direct call. Continuations join on every interpreter. Extra is
    /// empty. The Control file and line are the Location. Applies to debs and udebs.
    MaintainerScriptCallsInitScriptDirectly,
    code = "LN0198",
    tag = "maintainer-script-calls-init-script-directly",
    default_selected = true,
    experimental = false,
}

static INIT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\s*/etc/init\.d/(?:\S+)\s+["']?(?:\S+)["']?"#).expect("valid regex")
});
static INVOKE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*invoke-rc\.d\s+").expect("valid regex"));

impl BinaryPackageRule for MaintainerScriptCallsInitScriptDirectly {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        let lines = maintainer_script_lines(files, Continuations::Always);
        for (path, number) in direct_calls(&lines) {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: Some(number),
                column: None,
            }));
        }
    }
}

/// Last direct `/etc/init.d` call in each script that never starts `invoke-rc.d`.
fn direct_calls(lines: &[ScriptLine]) -> Vec<(PathBuf, u32)> {
    let mut hits = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let path = lines[index].path.clone();
        let mut init_line = None;
        let mut saw_invoke = false;
        while index < lines.len() && lines[index].path == path {
            let text = &lines[index].text;
            if INIT.is_match(text) {
                init_line = Some(lines[index].number);
            }
            if INVOKE.is_match(text) {
                saw_invoke = true;
            }
            index += 1;
        }
        if let Some(number) = init_line.filter(|_| !saw_invoke) {
            hits.push((path, number));
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(name: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            script("postinst", "#!/bin/sh\nset -e\n"),
            script("postinst", "#!/bin/sh\ninvoke-rc.d foo start\n"),
            script(
                "postinst",
                "#!/bin/sh\n/etc/init.d/foo start\ninvoke-rc.d foo stop\n",
            ),
            script(
                "postinst",
                "#!/bin/sh\nif which invoke-rc.d >/dev/null; then\n    invoke-rc.d package start\nelse\n    /etc/init.d/package start\nfi\n",
            ),
            script("postinst", "#!/bin/sh\n/etc/init.d/foo\n"),
            script("postinst", "/etc/init.d/foo start\n"),
            script("postinst", "#!/bin/sh\n# /etc/init.d/foo start\n"),
            script("postinst", "#!/bin/sh\necho /etc/init.d/foo start\n"),
            script("triggers", "#!/bin/sh\n/etc/init.d/foo start\n"),
        ];
        let fail = vec![
            script(
                "postinst",
                "#!/bin/sh\nset -e\n\nupdate-rc.d self-invoke defaults >/dev/null\n/etc/init.d/self-invoke start\n",
            ),
            script(
                "postinst",
                "#!/bin/sh\n/etc/init.d/foo start\n/etc/init.d/bar stop\n",
            ),
            script(
                "postinst",
                "#!/bin/sh\necho invoke-rc.d foo start\n/etc/init.d/foo start\n",
            ),
            script(
                "postinst",
                "#!/bin/sh\ninvoke-rc.d\n/etc/init.d/foo start\n",
            ),
            script("postinst", "#!/bin/sh\n    /etc/init.d/foo start\n"),
            script("postinst", "#!/bin/sh\n/etc/init.d/foo \"start\"\n"),
            script("postinst", "#!/bin/sh\n/etc/init.d/foo \\\nstart\n"),
            script("postrm", "#!/bin/sh\n/etc/init.d/package stop\n"),
            script("prerm", "#!/usr/bin/env bash\n/etc/init.d/package stop\n"),
        ];
        BinaryPackageTester::new(MaintainerScriptCallsInitScriptDirectly, pass, fail)
            .test_and_snapshot();
    }
}
