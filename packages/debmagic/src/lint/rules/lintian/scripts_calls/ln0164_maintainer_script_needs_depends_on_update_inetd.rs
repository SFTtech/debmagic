use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::script;

pub struct MaintainerScriptNeedsDependsOnUpdateInetd;

declare_rule! {
    /// A maintainer script calls `update-inetd` without naming a provider.
    ///
    /// Same Control files as `maintainer-script-needs-depends-on-adduser`.
    /// `postrm` is exempt, as is a binary package named `update-inetd`. The
    /// call is `update-inetd` followed by whitespace. `which`, `type`,
    /// `command`, `|| true`, and `-x` probes are ignored. The call is this Tag
    /// when neither `Depends` nor `Pre-Depends` names `update-inetd`,
    /// `inet-superserver`, `openbsd-inetd`, `inetutils-inetd`, `rlinetd`, or
    /// `xinetd`. A name anywhere in the field counts, including an alternative;
    /// this is not lintian's `satisfies`. Extra is `update-inetd (does not
    /// satisfy update-inetd | inet-superserver | openbsd-inetd |
    /// inetutils-inetd | rlinetd | xinetd)`. The Control file and line are the
    /// Location.
    MaintainerScriptNeedsDependsOnUpdateInetd,
    code = "LN0164",
    tag = "maintainer-script-needs-depends-on-update-inetd",
    default_selected = true,
    experimental = false,
}

static CALL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bupdate-inetd\s").expect("valid regex"));
static DASH_X: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"-x\s+\S*\bupdate-inetd\s").expect("valid regex"));
static WHICH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:which|type)\s+\bupdate-inetd\s").expect("valid regex"));
static COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"command\s+.*?\bupdate-inetd\s").expect("valid regex"));

const PRIMARY: &str = "update-inetd";
const PACKAGES: &[&str] = &[
    "update-inetd",
    "inet-superserver",
    "openbsd-inetd",
    "inetutils-inetd",
    "rlinetd",
    "xinetd",
];
const REQUIREMENT: &str =
    "update-inetd | inet-superserver | openbsd-inetd | inetutils-inetd | rlinetd | xinetd";

impl BinaryPackageRule for MaintainerScriptNeedsDependsOnUpdateInetd {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.package_name() == PRIMARY {
            return;
        }
        if script::strong_relation_mentions(ctx.installation_control().fields(), PACKAGES) {
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

    const CALLS: &str = "#!/bin/sh\nupdate-inetd --add foo\n";

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            postinst("#!/bin/sh\nset -e\ntrue\n"),
            postinst(CALLS).field("Depends", "update-inetd"),
            postinst(CALLS).field("Pre-Depends", "xinetd"),
            postinst(CALLS).field("Depends", "inet-superserver"),
            postinst(CALLS).field("Depends", "openbsd-inetd | inetutils-inetd"),
            postinst(CALLS).field("Depends", "rlinetd (>= 0.8)"),
            postinst(CALLS).package("update-inetd"),
            named("postrm", CALLS),
            postinst("#!/bin/sh\nupdate-inetd --add foo || true\n"),
            postinst("#!/bin/sh\nwhich update-inetd --help\n"),
            postinst("#!/bin/sh\n# update-inetd --add foo\n"),
            postinst("update-inetd --add foo\n"),
        ];
        let fail = vec![
            postinst(CALLS),
            postinst("#!/bin/sh\nset -e\nupdate-inetd --add foo\n"),
            // `netbase` used to ship update-inetd; the check does not accept it.
            postinst(CALLS).field("Depends", "netbase"),
            // The package-name exemption is only `update-inetd`, not every provider.
            postinst(CALLS).package("xinetd"),
            postinst(CALLS).field("Recommends", "update-inetd"),
            named("preinst", CALLS),
            postinst("#!/bin/sh\nupdate-inetd \\\n  --add foo\n"),
            postinst(CALLS).udeb(),
        ];
        BinaryPackageTester::new(MaintainerScriptNeedsDependsOnUpdateInetd, pass, fail)
            .test_and_snapshot();
    }
}
