use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;
use crate::lint::rules::lintian::init_scripts::systemd_unit;

pub struct SystemdServiceFileUsesDeprecatedSyslogFacility;

declare_rule! {
    /// A service unit's `[Service]` `StandardError=` or `StandardOutput=` is
    /// `syslog` or `syslog-console`.
    ///
    /// Keys are checked in the order `StandardError`, `StandardOutput`; for
    /// each, `syslog` then `syslog-console`. Each pair that matches is one
    /// finding, so a file can give two. Extra is `Key=value`. The unit path is
    /// the Location. Units are the regular files and hardlinks of the
    /// obsolete-target Tag; a masked `/dev/null` symbolic link is skipped.
    SystemdServiceFileUsesDeprecatedSyslogFacility,
    code = "LN0990",
    tag = "systemd-service-file-uses-deprecated-syslog-facility",
    default_selected = true,
    experimental = false,
}

const KEYS: &[&str] = &["StandardError", "StandardOutput"];
const FACILITIES: &[&str] = &["syslog", "syslog-console"];

impl BinaryPackageRule for SystemdServiceFileUsesDeprecatedSyslogFacility {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for (path, text) in systemd_unit::service_texts(files) {
            let lines = systemd_unit::service_lines(&text);
            for key in KEYS {
                let values = systemd_unit::service_values(&lines, "Service", key);
                for facility in FACILITIES {
                    if values.iter().any(|value| value == *facility) {
                        ctx.diagnostic(
                            Diagnostic::warn(format!("{key}={facility}"))
                                .with_location(at(path.clone())),
                        );
                    }
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
            unit(
                "usr/lib/systemd/system/journal.service",
                "[Service]\nStandardOutput=journal\nStandardError=journal\n",
            ),
            unit(
                "usr/lib/systemd/system/section.service",
                "[Unit]\nStandardError=syslog\n",
            ),
            unit(
                "usr/lib/systemd/system/cleared.service",
                "[Service]\nStandardOutput=syslog\nStandardOutput=\n",
            ),
            unit(
                "usr/lib/systemd/user/user.service",
                "[Service]\nStandardOutput=syslog\n",
            ),
            BinaryPackageCase::from(vec![(
                "usr/lib/systemd/system/masked.service".to_string(),
                TestFile::symlink("/dev/null"),
            )]),
        ];
        let fail = vec![
            unit(
                "usr/lib/systemd/system/bad.service",
                "[Service]\nStandardOutput=syslog\nStandardError=syslog-console\n",
            ),
            unit(
                "etc/systemd/system/copied.service",
                "[Service]\nStandardError=syslog\n",
            ),
            unit(
                "usr/lib/systemd/system/udeb.service",
                "[Service]\nStandardOutput=syslog-console\n",
            )
            .udeb(),
        ];
        BinaryPackageTester::new(SystemdServiceFileUsesDeprecatedSyslogFacility, pass, fail)
            .test_and_snapshot();
    }
}
