use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct Apache2ReverseDependencyCallsInvokeRcD;

declare_rule! {
    /// A maintainer script of an Apache reverse dependency calls `invoke-rc.d
    /// apache2`.
    ///
    /// The package ships a module under `usr/lib/apache2/modules/`, a file in
    /// `etc/apache2/{conf,site}-{available,enabled}/`, or `etc/apache2/conf.d/`.
    /// Only shell maintainer scripts are read, and a line that starts with `#`
    /// is a comment. Extra is empty, so the Location (the script and line) is
    /// part of the message. The web server packages are exempt.
    Apache2ReverseDependencyCallsInvokeRcD,
    code = "LN0848",
    tag = "apache2-reverse-dependency-calls-invoke-rc.d",
    default_selected = true,
    experimental = false,
}

static INVOKE_RC_D: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"invoke-rc\.d\s+apache2").expect("valid regex"));

impl BinaryPackageRule for Apache2ReverseDependencyCallsInvokeRcD {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if scan::is_web_server(ctx.package_name()) {
            return;
        }
        let Some(data) = ctx.data_files() else {
            return;
        };
        if !scan::is_related(data) {
            return;
        }
        let Some(control) = ctx.control_files() else {
            return;
        };
        let hits: Vec<_> = scan::shell_lines(control)
            .into_iter()
            .filter(|line| !line.text.starts_with('#') && INVOKE_RC_D.is_match(&line.text))
            .map(|line| (line.path, line.line))
            .collect();
        for (path, line) in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
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

    fn case(data: &[(&str, &str)], scripts: &[(&str, &str)]) -> BinaryPackageCase {
        let mut packaged = BinaryPackageCase::from(
            data.iter()
                .map(|(path, content)| (path.to_string(), TestFile::new(*content)))
                .collect::<Vec<_>>(),
        );
        for (name, content) in scripts {
            packaged = packaged.control_file(*name, TestFile::new(*content));
        }
        packaged
    }

    #[test]
    fn test() {
        let conf = [("etc/apache2/conf-available/foo.conf", "")];
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            case(&conf, &[("postinst", "#!/bin/sh\ntrue\n")]),
            case(
                &conf,
                &[("postinst", "#!/bin/sh\n# invoke-rc.d apache2 reload\n")],
            ),
            case(
                &[("usr/bin/foo", "")],
                &[("postinst", "#!/bin/sh\ninvoke-rc.d apache2 reload\n")],
            ),
            case(
                &conf,
                &[("postinst", "#!/usr/bin/perl\ninvoke-rc.d apache2 reload\n")],
            ),
            case(
                &conf,
                &[("postinst", "#!/bin/sh\ninvoke-rc.d apache2 reload\n")],
            )
            .package("apache2"),
        ];
        let fail = vec![
            case(
                &conf,
                &[
                    (
                        "postinst",
                        "#!/bin/sh\nset -e\ninvoke-rc.d apache2 reload\n",
                    ),
                    ("postrm", "#!/bin/sh\n\ninvoke-rc.d  apache2 stop\n"),
                ],
            ),
            case(
                &[("usr/lib/apache2/modules/mod_foo.so", "")],
                &[("prerm", "#!/bin/sh\n  invoke-rc.d apache2 stop\n")],
            ),
        ];
        BinaryPackageTester::new(Apache2ReverseDependencyCallsInvokeRcD, pass, fail)
            .test_and_snapshot();
    }
}
