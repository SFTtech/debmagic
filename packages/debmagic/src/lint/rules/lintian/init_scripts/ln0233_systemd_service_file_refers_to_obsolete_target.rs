use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::systemd_unit;

pub struct SystemdServiceFileRefersToObsoleteTarget;

declare_rule! {
    /// A systemd unit's `[Unit]` `After=` names an obsolete target.
    ///
    /// The systemd check hardcodes `syslog.target` and `dbus.target` (it does
    /// not load a data file). An empty assignment clears earlier values. A
    /// trailing `\` joins the next line, and shell words are split the way
    /// lintian splits them. Extra is the target. The unit path is the Location.
    SystemdServiceFileRefersToObsoleteTarget,
    code = "LN0233",
    tag = "systemd-service-file-refers-to-obsolete-target",
    default_selected = true,
    experimental = false,
}

/// Hardcoded in `Lintian::Check::Systemd`, not loaded from `data/`.
const OBSOLETE: &[&str] = &["syslog.target", "dbus.target"];

impl BinaryPackageRule for SystemdServiceFileRefersToObsoleteTarget {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let units = systemd_unit::service_texts(files);
        for (path, text) in units {
            let lines = systemd_unit::service_lines(&text);
            for target in systemd_unit::service_values(&lines, "Unit", "After") {
                if OBSOLETE.contains(&target.as_str()) {
                    ctx.diagnostic(Diagnostic::warn(target).with_location(at(path.clone())));
                }
            }
        }
    }
}

fn at(path: PathBuf) -> Location {
    Location {
        path,
        line: None,
        column: None,
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn unit(path: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new(body))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            unit(
                "usr/lib/systemd/system/ok.service",
                "[Unit]\nAfter=network.target\n",
            ),
            unit(
                "usr/lib/systemd/system/reset.service",
                "[Unit]\nAfter=syslog.target\nAfter=\n",
            ),
            unit(
                "usr/lib/systemd/system/wrong-section.service",
                "[Service]\nAfter=dbus.target\n",
            ),
            unit(
                "usr/lib/systemd/user/user.service",
                "[Unit]\nAfter=syslog.target\n",
            ),
            unit(
                "usr/lib/systemd/system/comment.service",
                "[Unit]\n# After=syslog.target\n",
            ),
            unit(
                "usr/lib/systemd/system/foo.socket",
                "[Unit]\nAfter=syslog.target\n",
            ),
            BinaryPackageCase::from(vec![(
                "usr/lib/systemd/system/masked.service".to_string(),
                TestFile::symlink("/dev/null"),
            )]),
        ];
        let fail = vec![
            unit(
                "usr/lib/systemd/system/syslog.service",
                "[Unit]\nAfter=syslog.target\n",
            ),
            unit(
                "usr/lib/systemd/system/dbus.service",
                "[Unit]\nAfter=dbus.target\n",
            ),
            unit(
                "usr/lib/systemd/system/both.service",
                "[Unit]\nAfter=syslog.target dbus.target\n",
            ),
            unit(
                "usr/lib/systemd/system/continued.service",
                "[Unit]\nAfter=network.target \\\nsyslog.target\n",
            ),
            unit(
                "usr/lib/systemd/system/reset-then.service",
                "[Unit]\nAfter=\nAfter=syslog.target\n",
            ),
            unit(
                "usr/lib/systemd/system/quoted.service",
                "[Unit]\nAfter=\"syslog.target\"\n",
            ),
            BinaryPackageCase::from(vec![
                (
                    "etc/systemd/system/copied.service".to_string(),
                    TestFile::new("[Unit]\nAfter=syslog.target\n"),
                ),
                (
                    "usr/lib/systemd/system/copied.service".to_string(),
                    TestFile::new("[Unit]\nAfter=syslog.target\n"),
                ),
            ]),
        ];
        BinaryPackageTester::new(SystemdServiceFileRefersToObsoleteTarget, pass, fail)
            .test_and_snapshot();
    }
}
