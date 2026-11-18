use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;
use crate::lint::rules::lintian::init_scripts::systemd_unit;

pub struct SystemdServiceAliasWithoutExtension;

declare_rule! {
    /// A service unit's `Install` `Alias=` name does not end in `.service`.
    ///
    /// The same unit paths, continuations, comments, and shell words as the
    /// obsolete-target Tag. One finding per alias that fails the suffix
    /// check, including an empty word from `Alias=""`. A trailing `.service`
    /// is case-sensitive. Extra is empty. The unit path is the Location.
    SystemdServiceAliasWithoutExtension,
    code = "LN0295",
    tag = "systemd-service-alias-without-extension",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SystemdServiceAliasWithoutExtension {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for (path, text) in systemd_unit::service_texts(files) {
            let lines = systemd_unit::service_lines(&text);
            for alias in systemd_unit::service_values(&lines, "Install", "Alias") {
                if !alias.ends_with(".service") {
                    ctx.diagnostic(Diagnostic::warn("").with_location(at(path.clone())));
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
                "[Install]\nAlias=foo.service\n",
            ),
            unit(
                "usr/lib/systemd/system/dot.service",
                "[Install]\nAlias=.service\n",
            ),
            unit(
                "usr/lib/systemd/system/reset.service",
                "[Install]\nAlias=bare\nAlias=\n",
            ),
            unit(
                "usr/lib/systemd/system/spaced.service",
                "[Install]\nAlias = bare\n",
            ),
            unit(
                "usr/lib/systemd/system/wrong.service",
                "[Unit]\nAlias=bare\n",
            ),
            unit(
                "usr/lib/systemd/user/user.service",
                "[Install]\nAlias=bare\n",
            ),
            unit(
                "usr/lib/systemd/system/foo.socket",
                "[Install]\nAlias=bare\n",
            ),
            BinaryPackageCase::from(vec![(
                "usr/lib/systemd/system/masked.service".to_string(),
                TestFile::symlink("/dev/null"),
            )]),
        ];
        let fail = vec![
            unit(
                "usr/lib/systemd/system/bare.service",
                "[Install]\nAlias=systemd-aliasd.service\nAlias=anothertest\n",
            ),
            unit(
                "usr/lib/systemd/system/two.service",
                "[Install]\nAlias=foo bar\n",
            ),
            unit(
                "usr/lib/systemd/system/case.service",
                "[Install]\nAlias=foo.Service\n",
            ),
            unit(
                "usr/lib/systemd/system/empty.service",
                "[Install]\nAlias=\"\"\n",
            ),
            unit(
                "usr/lib/systemd/system/continued.service",
                "[Install]\nAlias=foo.service \\\nbar\n",
            ),
            unit(
                "etc/systemd/system/copied.service",
                "[Install]\nAlias=bare\n",
            ),
            unit("lib/systemd/system/lib.service", "[Install]\nAlias=bare\n").udeb(),
        ];
        BinaryPackageTester::new(SystemdServiceAliasWithoutExtension, pass, fail)
            .test_and_snapshot();
    }
}
