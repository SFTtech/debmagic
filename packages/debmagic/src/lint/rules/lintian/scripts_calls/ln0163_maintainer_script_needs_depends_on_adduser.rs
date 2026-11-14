use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::script;

pub struct MaintainerScriptNeedsDependsOnAdduser;

declare_rule! {
    /// A maintainer script calls `adduser` without naming it in a strong dependency.
    ///
    /// Control files whose name is `config…` or ends in `preinst`, `postinst`,
    /// `prerm`, or `postrm`, and that are regular files with a shebang. `postrm`
    /// is exempt, as is a binary package named `adduser`. The call is `adduser`
    /// followed by whitespace, after comments are stripped and shell
    /// continuations are joined. `which`, `type`, `command`, `|| true`, and
    /// `-x` probes are ignored. The call is this Tag when neither `Depends` nor
    /// `Pre-Depends` names `adduser`. A name anywhere in the field counts,
    /// including an alternative and a version restriction; this is not lintian's
    /// `satisfies`. Extra is `adduser (does not satisfy adduser)`. The Control
    /// file and line are the Location.
    MaintainerScriptNeedsDependsOnAdduser,
    code = "LN0163",
    tag = "maintainer-script-needs-depends-on-adduser",
    default_selected = true,
    experimental = false,
}

static CALL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\badduser\s").expect("valid regex"));
static DASH_X: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"-x\s+\S*\badduser\s").expect("valid regex"));
static WHICH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:which|type)\s+\badduser\s").expect("valid regex"));
static COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"command\s+.*?\badduser\s").expect("valid regex"));

const PRIMARY: &str = "adduser";
const REQUIREMENT: &str = "adduser";

impl BinaryPackageRule for MaintainerScriptNeedsDependsOnAdduser {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.package_name() == PRIMARY {
            return;
        }
        if script::strong_relation_mentions(ctx.installation_control().fields(), &[PRIMARY]) {
            return;
        }
        let hits = {
            let Some(files) = ctx.control_files() else {
                return;
            };
            script::prerequisite_hits(files, &CALL, &DASH_X, &WHICH, &COMMAND)
        };
        for hit in hits {
            let message = format!("{} (does not satisfy {REQUIREMENT})", hit.command);
            ctx.diagnostic(Diagnostic::warn(message).with_location(Location {
                path: hit.path,
                line: Some(hit.line),
                column: None,
            }));
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

    fn named(name: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(body))
    }

    const CALLS: &str = "#!/bin/sh\nadduser --system foo\n";

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            postinst("#!/bin/sh\nset -e\ntrue\n"),
            postinst(CALLS).field("Depends", "adduser"),
            postinst(CALLS).field("Pre-Depends", "adduser"),
            postinst(CALLS).field("Depends", "adduser (>= 3.118)"),
            postinst(CALLS).field("Depends", "adduser:any"),
            // An alternative still counts: presence, not satisfies().
            postinst(CALLS).field("Depends", "adduser | something-else"),
            postinst(CALLS).package("adduser"),
            named("postrm", CALLS),
            postinst("#!/bin/sh\nwhich adduser foo\n"),
            postinst("#!/bin/sh\ntype adduser foo\n"),
            postinst("#!/bin/sh\ncommand -v adduser foo\n"),
            postinst("#!/bin/sh\nadduser foo || true\n"),
            postinst("#!/bin/sh\nif [ -x /usr/sbin/adduser ]; then true; fi\n"),
            postinst("#!/bin/sh\n# adduser --system foo\n"),
            postinst("adduser --system foo\n"),
            postinst("#!/bin/sh\nadduser \\\n"),
            // Not a shell, so `adduser\` is not joined with the next line.
            postinst("#!/usr/bin/perl\nadduser\\\n foo\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::symlink("foo")),
            named("triggers", "#!/bin/sh\nadduser foo\n"),
        ];
        let fail = vec![
            postinst(CALLS),
            postinst("#!/bin/sh\nset -e\nadduser --system foo\n"),
            named("preinst", CALLS),
            named("prerm", CALLS),
            named("config", CALLS),
            named("my-postrm", CALLS),
            postinst("#!/usr/bin/env sh\nadduser foo\n"),
            postinst("#!/usr/bin/perl\nadduser foo\n"),
            // Joined continuation is reported on the last physical line.
            postinst("#!/bin/sh\nadduser \\\n  --system foo\n"),
            // lintian deletes the first backslash, which splits `adduser` from `foo`.
            postinst("#!/bin/sh\nadduser\\ foo \\\nbar\n"),
            postinst(CALLS).field("Recommends", "adduser"),
            postinst(CALLS).field("Depends", "libc6"),
            postinst(CALLS).udeb(),
        ];
        BinaryPackageTester::new(MaintainerScriptNeedsDependsOnAdduser, pass, fail)
            .test_and_snapshot();
    }
}
