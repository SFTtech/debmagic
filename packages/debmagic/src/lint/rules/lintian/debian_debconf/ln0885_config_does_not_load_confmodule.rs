use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, scripts};

pub struct ConfigDoesNotLoadConfmodule;

declare_rule! {
    /// A `config` script does not load the debconf confmodule.
    ///
    /// `config` must be a regular file that starts with `#!`. It counts as
    /// loading when it has `.` or `source` of `/usr/share/debconf/confmodule`,
    /// `use` or `require` of `Debconf::Client::ConfModule`, or an obsolete
    /// name (`confmodule.sh`, `Debian::DebConf::Client::ConfModule`), which
    /// loads the same way. Comments are dropped and a trailing `\` joins the
    /// next line first. The Location is `config` with no line and the Extra is
    /// empty. A `config` without `#!` is skipped, and udebs are not checked.
    ConfigDoesNotLoadConfmodule,
    code = "LN0885",
    tag = "config-does-not-load-confmodule",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ConfigDoesNotLoadConfmodule {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let path = {
            let Some(tree) = ctx.control_files() else {
                return;
            };
            let Some(config) = scripts::scan(tree)
                .into_iter()
                .find(|script| script.path.file_name().is_some_and(|name| name == "config"))
            else {
                return;
            };
            let loaded = config
                .lines
                .iter()
                .any(|line| scripts::loads_confmodule(&line.text));
            (!loaded).then_some(config.path)
        };
        if let Some(path) = path {
            ctx.diagnostic(Diagnostic::warn("").with_location(files::located(path, None)));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn config(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("config", TestFile::new(body))
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            config("#!/bin/sh\n. /usr/share/debconf/confmodule\n"),
            config("#!/usr/bin/perl\nuse Debconf::Client::ConfModule;\n"),
            config("#!/bin/sh\n. /usr/share/debconf/confmodule.sh\n"),
            config("echo no shebang\n"),
            config("#!/bin/sh\necho\n").udeb(),
        ];
        let fail = vec![
            config("#!/bin/sh\necho\n"),
            config("#!/bin/sh\n# . /usr/share/debconf/confmodule\n"),
            config("#!/bin/sh\n. \"/usr/share/debconf/confmodule\"\n"),
        ];
        BinaryPackageTester::new(ConfigDoesNotLoadConfmodule, pass, fail).test_and_snapshot();
    }
}
