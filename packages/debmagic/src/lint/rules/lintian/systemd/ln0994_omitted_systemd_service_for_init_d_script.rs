use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::BinaryPackageRule;
use crate::lint::rules::lintian::init_scripts::systemd_unit;

use super::systemd_init::{self, whitelisted};

pub struct OmittedSystemdServiceForInitDScript;

declare_rule! {
    /// An `etc/init.d` script has no systemd unit of its name, while the
    /// package does ship some `*.service` unit.
    ///
    /// Names are the unit basenames (an `@` before `.service` dropped) and the
    /// `Install` `Alias=` names (a trailing `.service` dropped). A script whose
    /// name is in systemd's init whitelist, or that is an rcS script, is
    /// skipped, as are the `README`, `skeleton`, `rc` and `rcS` names and the
    /// `initscripts` package. When the package ships no `.service` at all, the
    /// missing-service Tag is the one that applies. Extra is the service name
    /// (a trailing `.sh` dropped). The script path is the Location.
    OmittedSystemdServiceForInitDScript,
    code = "LN0994",
    tag = "omitted-systemd-service-for-init.d-script",
    default_selected = true,
    experimental = false,
}

static SERVICE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/systemd/system/.*\.service$").expect("valid regex"));

impl BinaryPackageRule for OmittedSystemdServiceForInitDScript {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let (any_service, hits) = systemd_init::init_scripts(files, &package);
        if !any_service {
            return;
        }
        let provided = provided_names(files);
        for hit in hits {
            if hit.rc_s || whitelisted(&hit.service) || provided.contains(&hit.service) {
                continue;
            }
            ctx.diagnostic(Diagnostic::error(hit.service).with_location(Location {
                path: hit.path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Unit basenames and `Alias=` names, as lintian's `service_names` holds them.
fn provided_names(files: &FileTree) -> HashSet<String> {
    let mut names = HashSet::new();
    for entry in files.query().path(&SERVICE) {
        let Some(base) = entry.path().file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(stem) = base.strip_suffix(".service") else {
            continue;
        };
        names.insert(stem.strip_suffix('@').unwrap_or(stem).to_string());
    }
    for (_, text) in systemd_unit::service_texts(files) {
        let lines = systemd_unit::service_lines(&text);
        for alias in systemd_unit::service_values(&lines, "Install", "Alias") {
            let alias = alias.strip_suffix(".service").unwrap_or(alias.as_str());
            if !alias.is_empty() {
                names.insert(alias.to_string());
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn with_unit(script: &str, unit: Option<(&str, &str)>) -> BinaryPackageCase {
        let mut files = vec![(
            format!("etc/init.d/{script}"),
            TestFile::new("#!/bin/sh\nexit 0\n"),
        )];
        if let Some((path, body)) = unit {
            files.push((path.to_string(), TestFile::new(body)));
        }
        BinaryPackageCase::from(files)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            with_unit("other", None),
            with_unit(
                "other",
                Some((
                    "lib/systemd/system/other.service",
                    "[Service]\nExecStart=/bin/true\n",
                )),
            ),
            with_unit(
                "other.sh",
                Some(("lib/systemd/system/other@.service", "[Service]\n")),
            ),
            with_unit(
                "real",
                Some((
                    "lib/systemd/system/unit.service",
                    "[Install]\nAlias=real.service\n",
                )),
            ),
            with_unit(
                "procps",
                Some(("lib/systemd/system/unit.service", "[Service]\n")),
            ),
            with_unit(
                "other",
                Some(("lib/systemd/system/unit.service", "[Service]\n")),
            )
            .package("initscripts"),
            BinaryPackageCase::from(vec![
                (
                    "etc/init.d/boot".to_string(),
                    TestFile::new("#!/bin/sh\n# Default-Start: S\n"),
                ),
                (
                    "lib/systemd/system/unit.service".to_string(),
                    TestFile::new("[Service]\n"),
                ),
            ]),
        ];
        let fail = vec![
            with_unit(
                "other",
                Some(("lib/systemd/system/unit.service", "[Service]\n")),
            ),
            with_unit(
                "other.sh",
                Some(("lib/systemd/system/unit.service", "[Service]\n")),
            ),
            with_unit(
                "other",
                Some(("lib/systemd/system/unit.service", "[Service]\n")),
            )
            .udeb(),
        ];
        BinaryPackageTester::new(OmittedSystemdServiceForInitDScript, pass, fail)
            .test_and_snapshot();
    }
}
