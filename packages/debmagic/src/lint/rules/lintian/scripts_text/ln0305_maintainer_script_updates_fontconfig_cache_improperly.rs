use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan::{self, CommandPattern};

pub struct MaintainerScriptUpdatesFontconfigCacheImproperly;

declare_rule! {
    /// A shell maintainer script runs `fc-cache`.
    ///
    /// The command may be `/usr/bin/fc-cache`, where a shell command may
    /// start. Extra is the match in double quotes, with both ends trimmed, so
    /// a bare `fc-cache` is `"fc-cache"`. A here-document is still scanned.
    /// The package named `fontconfig` is exempt. Other interpreters are not
    /// scanned.
    MaintainerScriptUpdatesFontconfigCacheImproperly,
    code = "LN0305",
    tag = "maintainer-script-updates-fontconfig-cache-improperly",
    default_selected = true,
    experimental = false,
}

static COMMAND: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&scan::leading(r"(?:/usr/bin/)?fc-cache(?:\s|\z)")).expect("valid regex")
});
static EXCLUDE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^fontconfig$").expect("valid regex"));

const KIND: CommandPattern = CommandPattern {
    shell_only: true,
    skip_automatic: false,
    quote: true,
};

impl BinaryPackageRule for MaintainerScriptUpdatesFontconfigCacheImproperly {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if EXCLUDE.is_match(ctx.package_name()) {
            return;
        }
        let Some(files) = ctx.control_files() else {
            return;
        };
        for hit in scan::find_commands(files, &COMMAND, &KIND) {
            ctx.diagnostic(Diagnostic::warn(hit.message).with_location(Location {
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

    fn control(name: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(content))
    }

    const CALL: &str = "#!/bin/sh\nfc-cache\n";

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            control("postinst", "#!/bin/sh\nset -e\n"),
            control("postinst", "#!/bin/sh\n# fc-cache\n"),
            control("postinst", "#!/bin/sh\necho fc-cache\n"),
            control("postinst", "#!/usr/bin/perl\nfc-cache\n"),
            control("postinst", "fc-cache\n"),
            control("triggers", "#!/bin/sh\nfc-cache\n"),
            control("postinst", CALL).package("fontconfig"),
        ];
        let fail = vec![
            control(
                "postinst",
                "#!/bin/sh\nfc-cache\ncat <<EOF\nfc-cache\nEOF\n",
            ),
            control("postinst", "#!/bin/sh\n/usr/bin/fc-cache -f\n"),
            control("postinst", "#!/bin/sh\ntrue; fc-cache\n"),
            control("postinst", "#!/usr/bin/env bash\nfc-cache\n"),
            control("postinst", CALL).package("fontconfig-config"),
            control(
                "postinst",
                "#!/bin/sh\n# Automatically added by dh_dummy/12\nfc-cache\n# End automatically added section\n",
            ),
        ];
        BinaryPackageTester::new(MaintainerScriptUpdatesFontconfigCacheImproperly, pass, fail)
            .test_and_snapshot();
    }
}
