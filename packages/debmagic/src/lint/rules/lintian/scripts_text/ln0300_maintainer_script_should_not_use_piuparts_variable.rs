use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan::{self, CommandPattern};

pub struct MaintainerScriptShouldNotUsePiupartsVariable;

declare_rule! {
    /// A shell maintainer script names a `PIUPARTS_*` variable.
    ///
    /// `PIUPARTS_TEST`, `PIUPARTS_OBJECTS`, `PIUPARTS_PHASE`,
    /// `PIUPARTS_DISTRIBUTION`, `PIUPARTS_DISTRIBUTION_PREV`, and
    /// `PIUPARTS_DISTRIBUTION_NEXT`. Extra is that name in double quotes.
    /// The first match on a line is the one reported. Other interpreters
    /// are not scanned.
    MaintainerScriptShouldNotUsePiupartsVariable,
    code = "LN0300",
    tag = "maintainer-script-should-not-use-piuparts-variable",
    default_selected = true,
    experimental = false,
}

static COMMAND: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"PIUPARTS_(?:TEST|OBJECTS|PHASE|DISTRIBUTION(?:_PREV|_NEXT)?)\b")
        .expect("valid regex")
});

const KIND: CommandPattern = CommandPattern {
    shell_only: true,
    skip_automatic: false,
    quote: true,
};

impl BinaryPackageRule for MaintainerScriptShouldNotUsePiupartsVariable {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
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

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            control("postinst", "#!/bin/sh\nset -e\n"),
            control("postinst", "#!/bin/sh\necho $PIUPARTS_IS_AWESOME\n"),
            control(
                "postinst",
                "#!/bin/sh\necho $PIUPARTS_DISTRIBUTION_PREVIOUS\n",
            ),
            control("postinst", "#!/bin/sh\n# echo $PIUPARTS_TEST\n"),
            control("postinst", "#!/usr/bin/perl\necho $PIUPARTS_TEST\n"),
            control("postinst", "echo $PIUPARTS_TEST\n"),
            control("triggers", "#!/bin/sh\necho $PIUPARTS_TEST\n"),
        ];
        let fail = vec![
            control(
                "postinst",
                "#!/bin/sh\necho $PIUPARTS_TEST\necho ${PIUPARTS_OBJECTS}\necho ${PIUPARTS_PHASE}\necho ${PIUPARTS_DISTRIBUTION}\necho ${PIUPARTS_DISTRIBUTION_NEXT}\necho ${PIUPARTS_DISTRIBUTION_PREV}\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\necho $PIUPARTS_TEST $PIUPARTS_PHASE\n",
            ),
            control(
                "postinst",
                "#!/usr/bin/env bash\necho $PIUPARTS_TEST # bad\n",
            ),
            control(
                "postrm",
                "#!/bin/sh\n# Automatically added by dh_dummy/12\necho $PIUPARTS_PHASE\n# End automatically added section\n",
            ),
        ];
        BinaryPackageTester::new(MaintainerScriptShouldNotUsePiupartsVariable, pass, fail)
            .test_and_snapshot();
    }
}
