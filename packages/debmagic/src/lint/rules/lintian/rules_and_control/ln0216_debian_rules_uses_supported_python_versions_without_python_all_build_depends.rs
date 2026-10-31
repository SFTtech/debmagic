use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::control_text::{build_depends_all_names, names_package};
use super::rules_text::{read_rules, rules_lines};

pub struct DebianRulesUsesSupportedPythonVersionsWithoutPythonAllBuildDepends;

declare_rule! {
    /// `debian/rules` asks `py3versions` for the supported interpreters.
    ///
    /// Scans the logical lines of `debian/rules` (comments dropped, `\`
    /// continuations joined) for `py3versions` followed by `--supported` or a
    /// short option cluster that contains `s` (`-s`, `-vs`, `-sv`). Extra is
    /// that `py3versions` invocation. Location is `debian/rules` at the
    /// physical line where the logical line ends. The hit is suppressed when
    /// `Build-Depends`, `Build-Depends-Indep`, or `Build-Depends-Arch` names
    /// `python3-all`, `python3-all-dev`, or `python3-all-dbg` (the package
    /// name, not a version check). Applies to the Source tree and Source
    /// package. A missing, unreadable, or non-UTF-8 `debian/rules` is not
    /// scanned.
    DebianRulesUsesSupportedPythonVersionsWithoutPythonAllBuildDepends,
    code = "LN0216",
    tag = "debian-rules-uses-supported-python-versions-without-python-all-build-depends",
    default_selected = true,
    experimental = false,
}

const PYTHON_ALL: &[&str] = &["python3-all", "python3-all-dev", "python3-all-dbg"];

static SUPPORTED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(py3versions\s+(?:[\w\s-]*--supported|-\w*s\w*))").expect("valid regex")
});

impl SourceRule for DebianRulesUsesSupportedPythonVersionsWithoutPythonAllBuildDepends {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let names = ctx
            .debian_control()
            .map(|control| build_depends_all_names(control.source()))
            .unwrap_or_default();
        if PYTHON_ALL
            .iter()
            .any(|package| names_package(&names, package))
        {
            return;
        }
        let Some(contents) = ctx.files().and_then(read_rules) else {
            return;
        };
        for (number, line) in rules_lines(&contents) {
            let Some(found) = SUPPORTED.captures(&line) else {
                continue;
            };
            ctx.diagnostic(
                Diagnostic::warn(found[1].to_string()).with_location(rules_location(number)),
            );
        }
    }
}

fn rules_location(line: u32) -> Location {
    Location {
        path: PathBuf::from("debian/rules"),
        line: Some(line),
        column: None,
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n";

    fn package(control: &str, body: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(control)),
            (
                "debian/rules",
                TestFile::new(format!("#!/usr/bin/make -f\n\n%:\n\tdh $@\n\n{body}")),
            ),
        ]
    }

    fn with_build_depends(field: &str, value: &str) -> String {
        format!(
            "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
{field}: {value}\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n"
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            package(CONTROL, ""),
            package(CONTROL, "echo $(shell py3versions -v)\n"),
            package(CONTROL, "echo $(shell py3versions -i)\n"),
            package(CONTROL, "echo $(shell py3versions --installed)\n"),
            package(CONTROL, "echo $(shell py3versions --verbose)\n"),
            // A comment is not a call.
            package(CONTROL, "# py3versions -s\n"),
            package(CONTROL, "  # py3versions --supported\n"),
            package(
                &with_build_depends("Build-Depends", "python3-all"),
                "echo $(shell py3versions -s)\n",
            ),
            package(
                &with_build_depends("Build-Depends-Indep", "python3-all-dev"),
                "echo $(shell py3versions -sv)\n",
            ),
            package(
                &with_build_depends("Build-Depends-Arch", "python3-all-dbg:any"),
                "echo $(shell py3versions --supported)\n",
            ),
            // Name presence: a version and an architecture restriction still count.
            package(
                &with_build_depends("Build-Depends", "python3-all (>= 3.11)"),
                "echo $(shell py3versions -s)\n",
            ),
            package(
                &with_build_depends("Build-Depends", "python3-all [amd64]"),
                "echo $(shell py3versions -s)\n",
            ),
            // A different package, and a binary Depends, do not suppress.
            package(
                &with_build_depends("Build-Depends", "python3"),
                "echo $(shell py3versions -v)\n",
            ),
        ];
        let fail = vec![
            package(CONTROL, "echo $(shell py3versions -s)\n"),
            package(CONTROL, "echo $(shell py3versions -vs)\n"),
            package(CONTROL, "echo $(shell py3versions -sv)\n"),
            package(CONTROL, "echo $(shell py3versions --supported)\n"),
            package(CONTROL, "echo $(shell py3versions --verbose --supported)\n"),
            // Continuations join; the pointer is the last physical line.
            package(CONTROL, "echo $(shell py3versions \\\n\t-s)\n"),
            // A comment is dropped and does not break a continuation.
            package(
                CONTROL,
                "echo $(shell py3versions \\\n# comment\n\t--supported)\n",
            ),
            // One report per logical line, the first match on that line.
            package(CONTROL, "py3versions -s\npy3versions --supported\n"),
            // `python3-all` on the binary stanza, or only in Build-Conflicts.
            package(
                "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
Build-Conflicts: python3-all\n\
\n\
Package: example\n\
Architecture: all\n\
Depends: python3-all\n\
Description: example\n extra\n",
                "py3versions -s\n",
            ),
            package(
                &with_build_depends("Build-Depends", "python3-all-doc"),
                "py3versions -s\n",
            ),
        ];
        SourceTreeTester::new(
            DebianRulesUsesSupportedPythonVersionsWithoutPythonAllBuildDepends,
            pass,
            fail,
        )
        .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(
            CONTROL,
            "echo $(shell py3versions -v)\n",
        ))];
        let fail = vec![SourcePackageCase::from(package(
            CONTROL,
            "echo $(shell py3versions -s)\n",
        ))];
        SourcePackageTester::new(
            DebianRulesUsesSupportedPythonVersionsWithoutPythonAllBuildDepends,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
