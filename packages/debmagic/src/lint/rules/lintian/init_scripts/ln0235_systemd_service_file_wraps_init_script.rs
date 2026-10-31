use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::systemd_unit;

pub struct SystemdServiceFileWrapsInitScript;

declare_rule! {
    /// A service file invokes an init script from an `Exec*` key.
    ///
    /// Keys, in order: ExecStart, ExecStartPre, ExecStartPost, ExecReload,
    /// ExecStop, ExecStopPost, all in `[Service]`. One finding per key when
    /// any shell word starts with `/etc/init.d/`. An empty assignment clears
    /// that key. Extra is the key. The unit path is the Location.
    SystemdServiceFileWrapsInitScript,
    code = "LN0235",
    tag = "systemd-service-file-wraps-init-script",
    default_selected = true,
    experimental = false,
}

const WRAP_KEYS: &[&str] = &[
    "ExecStart",
    "ExecStartPre",
    "ExecStartPost",
    "ExecReload",
    "ExecStop",
    "ExecStopPost",
];

impl BinaryPackageRule for SystemdServiceFileWrapsInitScript {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let units = systemd_unit::service_texts(files);
        for (path, text) in units {
            let lines = systemd_unit::service_lines(&text);
            for key in WRAP_KEYS {
                let wraps = systemd_unit::service_values(&lines, "Service", key)
                    .iter()
                    .any(|word| word.starts_with("/etc/init.d/"));
                if wraps {
                    ctx.diagnostic(Diagnostic::warn(*key).with_location(at(path.clone())));
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
                "usr/lib/systemd/system/ok.service",
                "[Service]\nExecStart=/bin/true\n",
            ),
            unit(
                "usr/lib/systemd/system/reset.service",
                "[Service]\nExecStart=/etc/init.d/foo start\nExecStart=\n",
            ),
            unit(
                "usr/lib/systemd/system/dashed.service",
                "[Service]\nExecStart=-/etc/init.d/foo\n",
            ),
            unit(
                "usr/lib/systemd/system/wrong-section.service",
                "[Unit]\nExecStart=/etc/init.d/foo\n",
            ),
            BinaryPackageCase::from(vec![(
                "usr/lib/systemd/system/masked.service".to_string(),
                TestFile::symlink("/dev/null"),
            )]),
        ];
        let all_keys = "\
[Service]
ExecStart=/etc/init.d/test5 start
ExecStartPre=/etc/init.d/test5 start-pre
ExecStartPost=/etc/init.d/test5 start-post
ExecReload=/etc/init.d/test5 reload
ExecStop=/etc/init.d/test5 stop
ExecStopPost=/etc/init.d/test5 stop-post
";
        let fail = vec![
            unit(
                "usr/lib/systemd/system/wrap.service",
                "[Service]\nExecStart=/etc/init.d/foo start\n",
            ),
            unit("usr/lib/systemd/system/test5.service", all_keys),
            unit(
                "usr/lib/systemd/system/quoted.service",
                "[Service]\nExecStart=\"/etc/init.d/foo start\"\n",
            ),
            unit(
                "usr/lib/systemd/system/second-word.service",
                "[Service]\nExecStart=/bin/sh /etc/init.d/foo\n",
            ),
            BinaryPackageCase::from(vec![
                (
                    "usr/lib/systemd/system/a.service".to_string(),
                    TestFile::new("[Service]\nExecStop=/etc/init.d/a stop\n"),
                ),
                (
                    "usr/lib/systemd/system/b.service".to_string(),
                    TestFile::new("[Service]\nExecReload=/etc/init.d/b reload\n"),
                ),
            ]),
        ];
        BinaryPackageTester::new(SystemdServiceFileWrapsInitScript, pass, fail).test_and_snapshot();
    }
}
