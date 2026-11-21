use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;
use crate::lint::rules::lintian::init_scripts::systemd_unit;

pub struct KillModeNone;

declare_rule! {
    /// A service unit's `[Service]` `KillMode=` is `none`.
    ///
    /// Values are trimmed at both ends, and an empty assignment clears earlier
    /// ones. One finding per matching value. Extra is the unit path, as lintian
    /// reports it (the hint has no other text). The unit path is the Location.
    /// Units are the regular files and hardlinks of the obsolete-target Tag; a
    /// masked `/dev/null` symbolic link is skipped.
    KillModeNone,
    code = "LN0993",
    tag = "kill-mode-none",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for KillModeNone {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for (path, text) in systemd_unit::service_texts(files) {
            let lines = systemd_unit::service_lines(&text);
            for mode in systemd_unit::service_values(&lines, "Service", "KillMode") {
                if mode.trim() == "none" {
                    ctx.diagnostic(
                        Diagnostic::warn(path.to_string_lossy().into_owned())
                            .with_location(at(path.clone())),
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
                "usr/lib/systemd/system/mixed.service",
                "[Service]\nKillMode=control-group\n",
            ),
            unit(
                "usr/lib/systemd/system/section.service",
                "[Unit]\nKillMode=none\n",
            ),
            unit(
                "usr/lib/systemd/system/cleared.service",
                "[Service]\nKillMode=none\nKillMode=\n",
            ),
            unit(
                "usr/lib/systemd/user/user.service",
                "[Service]\nKillMode=none\n",
            ),
            BinaryPackageCase::from(vec![(
                "usr/lib/systemd/system/masked.service".to_string(),
                TestFile::symlink("/dev/null"),
            )]),
        ];
        let fail = vec![
            unit(
                "usr/lib/systemd/system/kill-mode-none.service",
                "[Service]\nKillMode=none\n",
            ),
            unit(
                "etc/systemd/system/trailing.service",
                "[Service]\nKillMode=none \n",
            ),
            unit(
                "usr/lib/systemd/system/udeb.service",
                "[Service]\nKillMode=none\n",
            )
            .udeb(),
        ];
        BinaryPackageTester::new(KillModeNone, pass, fail).test_and_snapshot();
    }
}
