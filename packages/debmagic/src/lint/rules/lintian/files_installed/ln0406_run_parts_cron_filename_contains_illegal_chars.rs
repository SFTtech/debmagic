use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::ln0394_file_in_usr_lib_sgml::{at, matching};

pub struct RunPartsCronFilenameContainsIllegalChars;

declare_rule! {
    /// A `run-parts` cron filename contains `.` or `+`.
    ///
    /// The path is `etc/cron.daily`, `hourly`, `monthly`, `weekly`, or
    /// `etc/cron.d`, then a filename that does not start with `.` and contains
    /// `+` or `.` somewhere after that first character. A leading dot
    /// (`.placeholder`) is not flagged. Extra is empty. The Location is the
    /// path. Udebs are checked.
    RunPartsCronFilenameContainsIllegalChars,
    code = "LN0406",
    tag = "run-parts-cron-filename-contains-illegal-chars",
    default_selected = true,
    experimental = false,
}

static CRON: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^etc/cron\.(?:daily|hourly|monthly|weekly|d)/[^.].*[+.]").expect("regex")
});

impl BinaryPackageRule for RunPartsCronFilenameContainsIllegalChars {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for path in matching(files, &CRON) {
            ctx.diagnostic(Diagnostic::warn("").with_location(at(path)));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn file(path: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new("#!/bin/sh\n"))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            file("etc/cron.daily/cronfile"),
            file("etc/cron.daily/.placeholder"),
            file("etc/cron.daily/.hidden.file"),
            file("etc/cron.hourly/script"),
            file("etc/crontab"),
            file("usr/bin/cron.daily.script"),
        ];
        let fail = vec![
            file("etc/cron.daily/cronfile-contains.fullstop"),
            file("etc/cron.hourly/cron-script.hourly"),
            file("etc/cron.weekly/job+extra"),
            file("etc/cron.monthly/a.b"),
            file("etc/cron.d/foo.bar"),
            file("etc/cron.d/foo+bar").udeb(),
        ];
        BinaryPackageTester::new(RunPartsCronFilenameContainsIllegalChars, pass, fail)
            .test_and_snapshot();
    }
}
