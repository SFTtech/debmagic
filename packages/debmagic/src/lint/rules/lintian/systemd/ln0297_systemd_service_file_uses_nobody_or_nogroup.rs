use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;
use crate::lint::rules::lintian::init_scripts::systemd_unit;

pub struct SystemdServiceFileUsesNobodyOrNogroup;

declare_rule! {
    /// A service unit runs as `User=nobody` or `Group=nogroup`.
    ///
    /// Both keys are read from `[Service]`. `User` is reported before
    /// `Group`. The match is the whole shell word, so `Nobody` and a
    /// `Group=nobody` do not count. An empty assignment clears earlier
    /// values. Extra is `User=nobody` or `Group=nogroup`. The unit path is
    /// the Location. The same files are read as for the obsolete-target Tag.
    SystemdServiceFileUsesNobodyOrNogroup,
    code = "LN0297",
    tag = "systemd-service-file-uses-nobody-or-nogroup",
    default_selected = true,
    experimental = false,
}

const BAD: &[(&str, &str)] = &[("User", "nobody"), ("Group", "nogroup")];

impl BinaryPackageRule for SystemdServiceFileUsesNobodyOrNogroup {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for (path, text) in systemd_unit::service_texts(files) {
            let lines = systemd_unit::service_lines(&text);
            for (key, value) in BAD {
                let values = systemd_unit::service_values(&lines, "Service", key);
                if values.iter().any(|item| item == value) {
                    ctx.diagnostic(
                        Diagnostic::warn(format!("{key}={value}")).with_location(at(path.clone())),
                    );
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
                "usr/lib/systemd/system/good.service",
                "[Service]\nUser=gooduser\nGroup=goodgroup\n",
            ),
            unit(
                "usr/lib/systemd/system/case.service",
                "[Service]\nUser=Nobody\nGroup=Nogroup\n",
            ),
            unit(
                "usr/lib/systemd/system/swapped.service",
                "[Service]\nUser=nogroup\nGroup=nobody\n",
            ),
            unit(
                "usr/lib/systemd/system/unit.service",
                "[Unit]\nUser=nobody\nGroup=nogroup\n",
            ),
            unit(
                "usr/lib/systemd/system/spaced.service",
                "[Service]\nUser = nobody\n",
            ),
            unit(
                "usr/lib/systemd/system/cleared.service",
                "[Service]\nUser=nobody\nUser=\n",
            ),
            unit(
                "usr/lib/systemd/user/user.service",
                "[Service]\nUser=nobody\n",
            ),
            BinaryPackageCase::from(vec![(
                "usr/lib/systemd/system/masked.service".to_string(),
                TestFile::symlink("/dev/null"),
            )]),
        ];
        let fail = vec![
            unit(
                "usr/lib/systemd/system/both.service",
                "[Service]\nUser=nobody\nGroup=nogroup\n",
            ),
            unit(
                "usr/lib/systemd/system/group.service",
                "[Service]\nUser=root\nGroup=nogroup\n",
            ),
            unit(
                "usr/lib/systemd/system/quoted.service",
                "[Service]\nUser=\"nobody\"\n",
            ),
            unit(
                "usr/lib/systemd/system/reset-then.service",
                "[Service]\nUser=\nUser=nobody\n",
            ),
            unit(
                "etc/systemd/system/copied.service",
                "[Service]\nGroup=nogroup\n",
            ),
            unit("lib/systemd/system/lib.service", "[Service]\nUser=nobody\n").udeb(),
        ];
        BinaryPackageTester::new(SystemdServiceFileUsesNobodyOrNogroup, pass, fail)
            .test_and_snapshot();
    }
}
