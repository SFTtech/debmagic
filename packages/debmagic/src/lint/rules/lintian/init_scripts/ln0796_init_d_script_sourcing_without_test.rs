use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct InitDScriptSourcingWithoutTest;

declare_rule! {
    /// An `etc/init.d` script sources a file in `/etc/default/` without first
    /// testing that the file exists.
    ///
    /// A body line that starts with `.` and a path under `/etc/default/` is a
    /// finding, unless an open `if` test is in effect. The test opens on a line
    /// that matches `if … test|[ … -e|-f|-r …` (`!` allowed), and closes on a
    /// line with the word `fi`. Both are checked on each body line, in order.
    /// Lines of the first LSB section do not count. Extra is the sourced path.
    /// The script path and the line are the Location. The same scripts are
    /// skipped as for the required-option Tag.
    InitDScriptSourcingWithoutTest,
    code = "LN0796",
    tag = "init.d-script-sourcing-without-test",
    default_selected = true,
    experimental = false,
}

static IF_TEST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bif\s+.*?(?:test|\[)(?:\s+!)?\s+-[efr]\s+").expect("valid regex")
});
static FI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bfi\b").expect("valid regex"));
static SOURCED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*\.\s+["']?(/etc/default/[$\w/-]+)"#).expect("valid regex"));

impl BinaryPackageRule for InitDScriptSourcingWithoutTest {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for script in scripts(ctx) {
            let mut in_file_test = false;
            for (line_number, line) in init_script::body_lines(&script.text) {
                if IF_TEST.is_match(line) {
                    in_file_test = true;
                }
                if FI.is_match(line) {
                    in_file_test = false;
                }
                if in_file_test {
                    continue;
                }
                if let Some(caps) = SOURCED.captures(line) {
                    ctx.diagnostic(
                        Diagnostic::error(caps[1].to_string())
                            .with_location(at(&script.path, line_number)),
                    );
                }
            }
        }
    }
}

fn scripts(ctx: &mut BinaryPackageContext<'_>) -> Vec<init_script::InitDScript> {
    let package = ctx.package_name().to_string();
    let registered = {
        let Some(control) = ctx.control_files() else {
            return Vec::new();
        };
        init_script::registered_names(control)
    };
    let Some(data) = ctx.data_files() else {
        return Vec::new();
    };
    init_script::init_d_scripts(data, &registered, &package)
}

fn at(path: &Path, line: u32) -> Location {
    Location {
        path: path.to_path_buf(),
        line: Some(line),
        column: None,
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(name: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(format!("etc/init.d/{name}"), TestFile::new(body))])
    }

    #[test]
    fn test() {
        let pass = vec![
            script(
                "guarded",
                "#!/bin/sh\nif [ -r /etc/default/foo ]; then\n. /etc/default/foo\nfi\n",
            ),
            script(
                "test",
                "#!/bin/sh\nif test -f /etc/default/foo; then\n. /etc/default/foo\nfi\n",
            ),
            script(
                "negated",
                "#!/bin/sh\nif [ ! -e /etc/default/foo ]; then\n. /etc/default/foo\nfi\n",
            ),
            script(
                "and-list",
                "#!/bin/sh\n[ -r /etc/default/foo ] && . /etc/default/foo\n",
            ),
            script("comment", "#!/bin/sh\n# . /etc/default/foo\n"),
            script("other-path", "#!/bin/sh\n. /etc/other\n"),
            script(
                "unterminated",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n. /etc/default/foo\n",
            ),
            script("pkg", "#!/bin/sh\n. /etc/default/foo\n").package("initscripts"),
        ];
        let fail = vec![
            script("plain", "#!/bin/sh\n. /etc/default/rcS\n"),
            script("quoted", "#!/bin/sh\n  . '/etc/default/foo'\n"),
            script(
                "after-fi",
                "#!/bin/sh\nif [ -r /etc/default/a ]; then\n. /etc/default/a\nfi\n. /etc/default/b\n",
            ),
            script("udeb-script", "#!/bin/sh\n. /etc/default/foo\n").udeb(),
        ];
        BinaryPackageTester::new(InitDScriptSourcingWithoutTest, pass, fail).test_and_snapshot();
    }
}
