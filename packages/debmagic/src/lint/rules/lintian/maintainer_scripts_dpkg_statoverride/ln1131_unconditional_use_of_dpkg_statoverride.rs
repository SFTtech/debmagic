use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::maintainer_script::{Continuations, maintainer_script_lines};
use crate::lint::rule::BinaryPackageRule;

pub struct UnconditionalUseOfDpkgStatoverride;

declare_rule! {
    /// A maintainer script calls `dpkg-statoverride --add` before any
    /// `dpkg-statoverride --list` in the same script.
    ///
    /// Only scripts with an interpreter (a shebang) are read, and `\` joins
    /// every script's lines. A logical line is a call when it starts a shell
    /// command (lintian's leading pattern: start of line, `&`, `;`, `(`, `|`,
    /// `{`, `if`, `then`, `do`, `while`, `!`, or `env VAR=…`), optionally
    /// with `/usr/bin/`, and is followed by whitespace. A `--list` on any
    /// earlier call line of that script, or on the same line, counts as
    /// checking the status first. Comment-only lines and trailing comments
    /// are ignored. Extra is empty; the Location is the last physical line of
    /// the call. The Rule reports only the `--add` call, one hit per line.
    ///
    /// Where we differ from lintian: the shared maintainer-script line walk
    /// reads regular files only, so a hardlinked or symlinked maintainer
    /// script is not checked.
    UnconditionalUseOfDpkgStatoverride,
    code = "LN1131",
    tag = "unconditional-use-of-dpkg-statoverride",
    default_selected = true,
    experimental = false,
}

static LEADING_PATTERN: &str = r#"(?:(?:^|[`&;(|{])\s*|(?:if|then|do|while|!)\s+|env(?:\s+[[:alnum:]_]+=(?:\S+|"[^"]*"|'[^']*'))*\s+)"#;

static STATOVERRIDE_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"{LEADING_PATTERN}(?:/usr/bin/)?dpkg-statoverride\s"
    ))
    .expect("valid regex")
});

impl BinaryPackageRule for UnconditionalUseOfDpkgStatoverride {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        let mut hits: Vec<(PathBuf, u32)> = Vec::new();
        let mut current: Option<PathBuf> = None;
        let mut saw_list = false;
        for line in maintainer_script_lines(files, Continuations::Always) {
            if current.as_ref() != Some(&line.path) {
                current = Some(line.path.clone());
                saw_list = false;
            }
            if !STATOVERRIDE_CALL.is_match(&line.text) {
                continue;
            }
            if line.text.contains("--list") {
                saw_list = true;
            }
            if line.text.contains("--add") && !saw_list {
                hits.push((line.path, line.number));
            }
        }
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

    fn with_script(name: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(content))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            with_script(
                "postinst",
                "#!/bin/sh\ndpkg-statoverride --list /x >/dev/null && true\ndpkg-statoverride --add root root 0755 /x\n",
            ),
            with_script(
                "postinst",
                "#!/bin/sh\ndpkg-statoverride --list x; dpkg-statoverride --add y\n",
            ),
            with_script("postinst", "dpkg-statoverride --add root root 0755 /x\n"),
            with_script("postinst", "#!/bin/sh\n# dpkg-statoverride --add x\n"),
            with_script("postinst", "#!/bin/sh\necho dpkg-statoverride --add x\n"),
            with_script("postinst", "#!/bin/sh\nfoo dpkg-statoverride --add x\n"),
            with_script("postinst", "#!/bin/sh\ndpkg-statoverride --remove /x\n"),
            with_script(
                "postinst",
                "#!/bin/sh\ndpkg-statoverride --list x\n# --add\n",
            ),
            with_script("triggers", "#!/bin/sh\ndpkg-statoverride --add x\n"),
            BinaryPackageCase::from(vec![(
                "usr/bin/example",
                TestFile::new("#!/bin/sh\ndpkg-statoverride --add x\n"),
            )]),
        ];
        let fail = vec![
            with_script(
                "postinst",
                "#!/bin/sh\ndpkg-statoverride --add root root 0755 /x\n",
            ),
            with_script(
                "postinst",
                "#!/bin/sh\nset -e\n  dpkg-statoverride --add x\n",
            ),
            // The `--list` comes after the `--add`.
            with_script(
                "postinst",
                "#!/bin/sh\ndpkg-statoverride --add x\ndpkg-statoverride --list x\n",
            ),
            // A `--list` in another script does not count.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file(
                    "postinst",
                    TestFile::new("#!/bin/sh\ndpkg-statoverride --list x\n"),
                )
                .control_file(
                    "postrm",
                    TestFile::new("#!/bin/sh\ndpkg-statoverride --add x\n"),
                ),
            with_script(
                "postinst",
                "#!/bin/sh\nif true; then /usr/bin/dpkg-statoverride --add x; fi\n",
            ),
            with_script("postinst", "#!/bin/sh\ndpkg-statoverride \\\n  --add x\n"),
            // A continuation joins in a non-shell script too.
            with_script(
                "postinst",
                "#!/usr/bin/perl\ndpkg-statoverride \\\n --add x\n",
            ),
            with_script(
                "postinst",
                "#!/bin/sh\ndpkg-statoverride --add x\ndpkg-statoverride --add y\n",
            ),
            with_script("postrm", "#!/bin/sh\ndpkg-statoverride --add x\n"),
            with_script("postinst", "#!/bin/sh\ndpkg-statoverride --add x\n").udeb(),
            // A trailing comment is removed before the `--list` check.
            with_script(
                "postinst",
                "#!/bin/sh\ndpkg-statoverride --add x # --list\n",
            ),
        ];
        BinaryPackageTester::new(UnconditionalUseOfDpkgStatoverride, pass, fail)
            .test_and_snapshot();
    }
}
