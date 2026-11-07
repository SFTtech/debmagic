use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct Apache2ReverseDependencyCallsWrapperScript;

declare_rule! {
    /// A shell maintainer script of an Apache reverse dependency calls
    /// `a2enmod` or one of its siblings.
    ///
    /// The package ships a module under `usr/lib/apache2/modules/`, a file in
    /// `etc/apache2/{conf,site}-{available,enabled}/`, or
    /// `etc/apache2/conf.d/`. The web server packages themselves are exempt.
    /// A line that starts with `#` is a comment. Extra is the command. The
    /// Location is that physical line.
    Apache2ReverseDependencyCallsWrapperScript,
    code = "LN0390",
    tag = "apache2-reverse-dependency-calls-wrapper-script",
    default_selected = true,
    experimental = false,
}

static WRAPPER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(a2(?:en|dis)(?:conf|site|mod))\b").expect("valid regex"));

impl BinaryPackageRule for Apache2ReverseDependencyCallsWrapperScript {
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
            .filter(|line| !line.text.starts_with('#'))
            .filter_map(|line| {
                let command = WRAPPER.captures(&line.text)?.get(1)?.as_str().to_string();
                Some((line.path, line.line, command))
            })
            .collect();
        for (path, line, command) in hits {
            ctx.diagnostic(Diagnostic::warn(command).with_location(Location {
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
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            case(
                &[("etc/apache2/conf-available/foo.conf", "")],
                &[("postinst", "#!/bin/sh\ntrue\n")],
            ),
            case(
                &[("etc/apache2/conf-available/foo.conf", "")],
                &[("postinst", "#!/bin/sh\n# a2enconf foo\n")],
            ),
            case(
                &[("usr/bin/foo", "")],
                &[("postinst", "#!/bin/sh\na2enmod foo\n")],
            ),
            case(
                &[("etc/apache2/conf-available/foo.conf", "")],
                &[("postinst", "#!/usr/bin/perl\na2enconf foo\n")],
            ),
            case(
                &[("etc/apache2/conf-available/foo.conf", "")],
                &[("postinst", "#!/bin/sh\na2enmod foo\n")],
            )
            .package("apache2"),
        ];
        let fail = vec![
            case(
                &[("etc/apache2/conf-available/foo.conf", "")],
                &[
                    ("postinst", "#!/bin/sh\nset -e\na2enconf foo\n"),
                    ("postrm", "#!/bin/sh\na2disconf foo\n"),
                ],
            ),
            case(
                &[("usr/lib/apache2/modules/mod_foo.so", "")],
                &[("postinst", "#!/bin/sh\na2enmod foo\n")],
            ),
        ];
        BinaryPackageTester::new(Apache2ReverseDependencyCallsWrapperScript, pass, fail)
            .test_and_snapshot();
    }
}
