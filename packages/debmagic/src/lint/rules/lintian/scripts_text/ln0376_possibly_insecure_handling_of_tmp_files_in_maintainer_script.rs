use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct PossiblyInsecureHandlingOfTmpFilesInMaintainerScript;

declare_rule! {
    /// A maintainer script touches `/tmp`, `/var/tmp`, or `$TMPDIR/…` without
    /// creating the file safely.
    ///
    /// Any interpreter. A logical line (continuations joined, comments
    /// removed) is reported when it names one of those paths and does not
    /// call `mktemp`, `mkstemp`, `tempfile`, or `mkdir`, and does not contain
    /// `XXXXXX` or `$RANDOM`. Extra is the path indicator. The Location is
    /// the last physical line of that logical line.
    PossiblyInsecureHandlingOfTmpFilesInMaintainerScript,
    code = "LN0376",
    tag = "possibly-insecure-handling-of-tmp-files-in-maintainer-script",
    default_selected = true,
    experimental = false,
}

static INDICATOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\W((?:/var)?/tmp|\$TMPDIR/[^)\]\}\s]+)").expect("valid regex"));
static SAFE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bmks?temp\b|\btempfile\b|\bmkdir\b|\bXXXXXX\b|\$RANDOM").expect("valid regex")
});

impl BinaryPackageRule for PossiblyInsecureHandlingOfTmpFilesInMaintainerScript {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        let hits: Vec<_> = scan::script_lines(files)
            .into_iter()
            .filter(|line| !SAFE.is_match(&line.text))
            .filter_map(|line| {
                let indicator = INDICATOR.captures(&line.text)?.get(1)?.as_str().to_string();
                Some((line.path, line.line, indicator))
            })
            .collect();
        for (path, line, indicator) in hits {
            ctx.diagnostic(Diagnostic::warn(indicator).with_location(Location {
                path,
                line: Some(line),
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
            control("postinst", "#!/bin/sh\nmktemp /tmp/foo\n"),
            control("postinst", "#!/bin/sh\nmkstemp /tmp/foo\n"),
            control("postinst", "#!/bin/sh\ntempfile /var/tmp/foo\n"),
            control("postinst", "#!/bin/sh\nmkdir /tmp/foo\n"),
            control("postinst", "#!/bin/sh\necho /tmp/foo.XXXXXX\n"),
            control("postinst", "#!/bin/sh\necho $RANDOM /tmp/foo\n"),
            control("postinst", "#!/bin/sh\n# cp /tmp/foo /x\n"),
            control("postinst", "cp /tmp/foo /x\n"),
            control("triggers", "#!/bin/sh\ncp /tmp/foo /x\n"),
        ];
        let fail = vec![
            control(
                "postinst",
                "#!/bin/sh\ncp /tmp/foo /x\ncp /var/tmp/bar /x\n",
            ),
            control("postrm", "#!/usr/bin/perl\nrm $TMPDIR/foo\n"),
            control("postinst", "#!/bin/sh\necho /tmp/a /var/tmp/b\n"),
        ];
        BinaryPackageTester::new(
            PossiblyInsecureHandlingOfTmpFilesInMaintainerScript,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
