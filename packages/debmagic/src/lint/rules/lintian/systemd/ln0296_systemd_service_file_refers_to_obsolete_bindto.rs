use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;
use crate::lint::rules::lintian::init_scripts::systemd_unit;

pub struct SystemdServiceFileRefersToObsoleteBindto;

declare_rule! {
    /// A service unit's `[Unit]` section still uses `BindTo=`.
    ///
    /// Any remaining shell word after empty assignments makes one finding
    /// for the file. A value in another section does not. Extra is empty.
    /// The unit path is the Location. The same files are read as for the
    /// obsolete-target Tag.
    SystemdServiceFileRefersToObsoleteBindto,
    code = "LN0296",
    tag = "systemd-service-file-refers-to-obsolete-bindto",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SystemdServiceFileRefersToObsoleteBindto {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for (path, text) in systemd_unit::service_texts(files) {
            let lines = systemd_unit::service_lines(&text);
            if !systemd_unit::service_values(&lines, "Unit", "BindTo").is_empty() {
                ctx.diagnostic(Diagnostic::warn("").with_location(at(path)));
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
                "[Unit]\nAfter=network.target\n",
            ),
            unit(
                "usr/lib/systemd/system/cleared.service",
                "[Unit]\nBindTo=foo\nBindTo=\n",
            ),
            unit("usr/lib/systemd/system/empty.service", "[Unit]\nBindTo=\n"),
            unit(
                "usr/lib/systemd/system/service.service",
                "[Service]\nBindTo=foo\n",
            ),
            unit(
                "usr/lib/systemd/system/spaced.service",
                "[Unit]\nBindTo = foo\n",
            ),
            unit("usr/lib/systemd/user/user.service", "[Unit]\nBindTo=foo\n"),
            BinaryPackageCase::from(vec![(
                "usr/lib/systemd/system/masked.service".to_string(),
                TestFile::symlink("/dev/null"),
            )]),
        ];
        let fail = vec![
            unit(
                "usr/lib/systemd/system/bound.service",
                "[Unit]\nBindTo=foo\n",
            ),
            unit(
                "usr/lib/systemd/system/many.service",
                "[Unit]\nBindTo=foo bar\n",
            ),
            unit(
                "usr/lib/systemd/system/reset-then.service",
                "[Unit]\nBindTo=\nBindTo=foo\n",
            ),
            unit(
                "etc/systemd/system/copied.service",
                "[Unit]\nAfter=network.target \\\nsyslog.target\nBindTo=foo\n",
            ),
            unit("lib/systemd/system/lib.service", "[Unit]\nBindTo=foo\n").udeb(),
        ];
        BinaryPackageTester::new(SystemdServiceFileRefersToObsoleteBindto, pass, fail)
            .test_and_snapshot();
    }
}
