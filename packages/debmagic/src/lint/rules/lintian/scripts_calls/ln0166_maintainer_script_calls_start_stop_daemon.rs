use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::script;

pub struct MaintainerScriptCallsStartStopDaemon;

declare_rule! {
    /// A shell maintainer script calls `start-stop-daemon` other than to stop.
    ///
    /// Same Control files as the other maintainer-script command Tags, but only
    /// when the shebang names a shell (`sh`, `bash`, `dash`, and the other
    /// basenames lintian treats as shells). The command is `start-stop-daemon`
    /// followed by whitespace. `--stop` anywhere after that command suppresses
    /// the Tag, including `start-stop-daemon--stop` which is not a call. Extra
    /// is `"start-stop-daemon"`. The Control file and line are the Location.
    MaintainerScriptCallsStartStopDaemon,
    code = "LN0166",
    tag = "maintainer-script-calls-start-stop-daemon",
    default_selected = true,
    experimental = false,
}

static CALL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bstart-stop-daemon\s").expect("valid regex"));
static STOP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s--stop\b").expect("valid regex"));

impl BinaryPackageRule for MaintainerScriptCallsStartStopDaemon {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let hits = {
            let Some(files) = ctx.control_files() else {
                return;
            };
            script::maintainer_script_lines(files)
                .into_iter()
                .filter(|line| line.shell)
                .filter_map(|line| {
                    let found = CALL.find(&line.text)?;
                    let after = &line.text[found.start() + "start-stop-daemon".len()..];
                    if STOP.is_match(after) {
                        return None;
                    }
                    Some(script::CommandHit {
                        path: line.path,
                        line: line.line,
                        command: "start-stop-daemon".to_string(),
                    })
                })
                .collect::<Vec<_>>()
        };
        for hit in hits {
            ctx.diagnostic(
                Diagnostic::warn("\"start-stop-daemon\"").with_location(Location {
                    path: hit.path,
                    line: Some(hit.line),
                    column: None,
                }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn postinst(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("postinst", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            postinst("#!/bin/sh\nset -e\ntrue\n"),
            postinst(
                "#!/bin/sh\nstart-stop-daemon --stop --quiet --name foo --startas /usr/bin/foo\n",
            ),
            postinst(
                "#!/bin/sh\nstart-stop-daemon --quiet --stop --name foo --startas /usr/bin/foo\n",
            ),
            postinst("#!/bin/sh\nstart-stop-daemon--stop --quiet --name foo\n"),
            postinst("#!/bin/sh\nstart-stop-daemon --start; start-stop-daemon --stop\n"),
            postinst("#!/usr/bin/perl\nstart-stop-daemon --start --name foo\n"),
            postinst("start-stop-daemon --start\n"),
            postinst("#!/bin/sh\n# start-stop-daemon --start\n"),
        ];
        let fail = vec![
            postinst(
                "#!/bin/sh\nstart-stop-daemon --start --quiet --name foo --startas /usr/bin/foo\n",
            ),
            postinst(
                "#!/bin/sh\nstart-stop-daemon --quiet --start --name foo --startas /usr/bin/foo\n",
            ),
            postinst("#!/usr/bin/env sh\nstart-stop-daemon --start --name foo\n"),
            postinst("#!/bin/bash\nstart-stop-daemon --start --name foo\n"),
            postinst("#!/bin/sh\n/sbin/start-stop-daemon --start --name foo\n"),
            postinst("#!/bin/sh\nstart-stop-daemon --start\n").udeb(),
        ];
        BinaryPackageTester::new(MaintainerScriptCallsStartStopDaemon, pass, fail)
            .test_and_snapshot();
    }
}
