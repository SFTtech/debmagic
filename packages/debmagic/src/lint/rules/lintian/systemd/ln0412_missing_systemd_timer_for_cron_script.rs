use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct MissingSystemdTimerForCronScript;

declare_rule! {
    /// A cron script has no systemd timer in the same package.
    ///
    /// Direct children of `etc/cron.*` (including `etc/cron.d`) are cron scripts.
    /// A timer is `(usr/)lib/systemd/system/*.timer`. When any timer is present,
    /// nothing is reported. Extra is empty. The script path is the Location.
    /// Udebs are checked too.
    MissingSystemdTimerForCronScript,
    code = "LN0412",
    tag = "missing-systemd-timer-for-cron-script",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MissingSystemdTimerForCronScript {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut cron = Vec::new();
        let mut timer = false;
        for entry in files.entries() {
            let Some(path) = entry.path().to_str() else {
                continue;
            };
            if is_timer(path) {
                timer = true;
            }
            if is_cron_script(path) {
                cron.push(PathBuf::from(path));
            }
        }
        if timer {
            return;
        }
        cron.sort();
        for path in cron {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn is_cron_script(path: &str) -> bool {
    let Some(parent) = path.rsplit_once('/') else {
        return false;
    };
    let Some(name) = parent.0.strip_prefix("etc/cron.") else {
        return false;
    };
    !name.is_empty() && !name.contains('/')
}

fn is_timer(path: &str) -> bool {
    let Some(name) = path
        .strip_prefix("usr/lib/systemd/system/")
        .or_else(|| path.strip_prefix("lib/systemd/system/"))
    else {
        return false;
    };
    !name.contains('/') && name.ends_with(".timer") && name != ".timer"
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn cron(name: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(name.to_string(), TestFile::new("#!/bin/sh\n"))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            cron("etc/cron/hourly/job"),
            cron("etc/cron.daily/nested/job"),
            BinaryPackageCase::from(vec![
                (
                    "etc/cron.daily/job".to_string(),
                    TestFile::new("#!/bin/sh\n"),
                ),
                (
                    "lib/systemd/system/job.timer".to_string(),
                    TestFile::new("[Timer]\n"),
                ),
            ]),
        ];
        let fail = vec![
            cron("etc/cron.daily/job"),
            cron("etc/cron.d/job"),
            BinaryPackageCase::from(vec![
                ("etc/cron.hourly/a".to_string(), TestFile::new("a\n")),
                ("etc/cron.weekly/b".to_string(), TestFile::new("b\n")),
                (
                    "usr/lib/systemd/system/not-a-timer.service".to_string(),
                    TestFile::new("[Service]\n"),
                ),
            ]),
        ];
        BinaryPackageTester::new(MissingSystemdTimerForCronScript, pass, fail).test_and_snapshot();
    }
}
