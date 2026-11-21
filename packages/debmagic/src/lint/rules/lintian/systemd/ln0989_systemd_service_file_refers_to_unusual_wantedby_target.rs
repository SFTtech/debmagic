use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;
use crate::lint::rules::lintian::init_scripts::systemd_unit;

use super::targets::USUAL_TARGETS;

pub struct SystemdServiceFileRefersToUnusualWantedbyTarget;

declare_rule! {
    /// A service unit's `[Install]` `WantedBy=` names a target outside the usual
    /// list (`default`, `graphical`, `multi-user`, `network-online`, `sleep`,
    /// `sysinit`).
    ///
    /// Units are `*.service` under `usr/lib/systemd/system` or
    /// `etc/systemd/system`, read as regular files and hardlinks; a symbolic
    /// link (the masked `/dev/null` form) is skipped. The package `systemd` is
    /// exempt. One finding per unusual word, in order, duplicates included.
    /// Extra is the target. The unit path is the Location. Udebs are checked.
    SystemdServiceFileRefersToUnusualWantedbyTarget,
    code = "LN0989",
    tag = "systemd-service-file-refers-to-unusual-wantedby-target",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SystemdServiceFileRefersToUnusualWantedbyTarget {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.package_name() == "systemd" {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        for (path, text) in systemd_unit::service_texts(files) {
            let lines = systemd_unit::service_lines(&text);
            for target in systemd_unit::service_values(&lines, "Install", "WantedBy") {
                if !USUAL_TARGETS.contains(&target.as_str()) {
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
            unit(
                "usr/lib/systemd/system/good.service",
                "[Install]\nWantedBy=multi-user.target\n",
            ),
            unit(
                "usr/lib/systemd/system/several.service",
                "[Install]\nWantedBy=graphical.target sysinit.target\n",
            ),
            unit(
                "usr/lib/systemd/system/unit.service",
                "[Unit]\nWantedBy=unusual.target\n",
            ),
            unit(
                "usr/lib/systemd/system/cleared.service",
                "[Install]\nWantedBy=unusual.target\nWantedBy=\n",
            ),
            unit(
                "usr/lib/systemd/user/user.service",
                "[Install]\nWantedBy=unusual.target\n",
            ),
            unit(
                "usr/lib/systemd/system/systemd-own.service",
                "[Install]\nWantedBy=unusual.target\n",
            )
            .package("systemd"),
            BinaryPackageCase::from(vec![(
                "etc/systemd/system/masked.service".to_string(),
                TestFile::symlink("/dev/null"),
            )]),
        ];
        let fail = vec![
            unit(
                "usr/lib/systemd/system/odd.service",
                "[Install]\nWantedBy=unusual.target\n",
            ),
            unit(
                "etc/systemd/system/odd.service",
                "[Install]\nWantedBy=unusual.target\n",
            ),
            unit(
                "usr/lib/systemd/system/mixed.service",
                "[Install]\nWantedBy=multi-user.target unusual.target\n",
            ),
            unit(
                "usr/lib/systemd/system/udeb.service",
                "[Install]\nWantedBy=unusual.target\n",
            )
            .udeb(),
        ];
        BinaryPackageTester::new(SystemdServiceFileRefersToUnusualWantedbyTarget, pass, fail)
            .test_and_snapshot();
    }
}
