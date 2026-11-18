use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MaintainerScriptModifiesInetdConf;

declare_rule! {
    /// A shell maintainer script rewrites `/etc/inetd.conf`.
    ///
    /// A `>` redirect onto that path, or a whole line that is `cp` or `mv`
    /// whose last argument is that path. Both can match one line, redirect
    /// first. Extra is empty. Copying the file somewhere else does not
    /// match. `Provides: inet-superserver` is not consulted. Other
    /// interpreters are not scanned.
    MaintainerScriptModifiesInetdConf,
    code = "LN0190",
    tag = "maintainer-script-modifies-inetd-conf",
    default_selected = true,
    experimental = false,
}

static REDIRECT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r">\s*/etc/inetd\.conf(?:\s|\z)").expect("valid regex"));
static COPY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?:cp|mv)\s+(?:.*\s)?/etc/inetd\.conf\s*$").expect("valid regex")
});

impl BinaryPackageRule for MaintainerScriptModifiesInetdConf {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        let hits: Vec<_> = scan::script_lines(files)
            .into_iter()
            .filter(|line| line.shell)
            .flat_map(|line| {
                let redirect = REDIRECT.is_match(&line.text);
                let copy = COPY.is_match(&line.text);
                let path = line.path;
                let number = line.line;
                [redirect, copy]
                    .into_iter()
                    .filter(|matches| *matches)
                    .map(move |_| (path.clone(), number))
            })
            .collect();
        for (path, line) in hits {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn control(name: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(content))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            control("postinst", "#!/bin/sh\nset -e\n"),
            control(
                "postinst",
                "#!/bin/sh\ncp /etc/inetd.conf /srv/chroot/etc/inetd.conf\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\ncp /etc/inetd.conf /etc/inetd.conf.bak\n",
            ),
            control("postinst", "#!/bin/sh\n# echo broken > /etc/inetd.conf\n"),
            control(
                "postinst",
                "#!/usr/bin/perl\necho 'broken' > /etc/inetd.conf\n",
            ),
            control("postinst", "echo 'broken' > /etc/inetd.conf\n"),
            control("triggers", "#!/bin/sh\necho 'broken' > /etc/inetd.conf\n"),
        ];
        let fail = vec![
            control(
                "postinst",
                "#!/bin/sh\necho 'broken' > /etc/inetd.conf\ncp /etc/inetd.conf.new /etc/inetd.conf\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\ncp /etc/inetd.conf.new /etc/inetd.conf > /etc/inetd.conf\n",
            ),
            control("postinst", "#!/bin/sh\necho x > /etc/inetd.conf extra\n"),
            control(
                "postinst",
                "#!/usr/bin/env sh\nmv /tmp/inetd.conf /etc/inetd.conf\n",
            ),
        ];
        BinaryPackageTester::new(MaintainerScriptModifiesInetdConf, pass, fail).test_and_snapshot();
    }
}
