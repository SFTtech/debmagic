use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::script::{Continuations, maintainer_script_lines};

pub struct MaintainerScriptShouldNotParseEtcPasswdOrGroup;

declare_rule! {
    /// A shell maintainer script parses `/etc/passwd` or `/etc/group` with grep.
    ///
    /// The interpreter basename is a shell (`sh` and the `ash` / `bash` / `dash` /
    /// `csh` / `ksh` / `zsh` family). The match is `grep`, `egrep`, or `fgrep`
    /// followed later on the line by `/etc/passwd` or `/etc/group`. Extra is that
    /// match in double quotes. The Control file and line are the Location.
    /// Applies to debs and udebs.
    MaintainerScriptShouldNotParseEtcPasswdOrGroup,
    code = "LN0199",
    tag = "maintainer-script-should-not-parse-etc-passwd-or-group",
    default_selected = true,
    experimental = false,
}

static GREP_PASSWD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b([ef]?grep\b.*/etc/(?:passwd|group))\b").expect("valid regex"));

impl BinaryPackageRule for MaintainerScriptShouldNotParseEtcPasswdOrGroup {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        let hits: Vec<_> = maintainer_script_lines(files, Continuations::ShellOnly)
            .into_iter()
            .filter(|line| line.shell)
            .filter_map(|line| {
                let found = GREP_PASSWD.find(&line.text)?;
                let text = found.as_str().trim();
                if text.is_empty() {
                    None
                } else {
                    Some((line.path, line.number, format!("\"{text}\"")))
                }
            })
            .collect();
        for (path, number, message) in hits {
            ctx.diagnostic(Diagnostic::warn(message).with_location(Location {
                path,
                line: Some(number),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(name: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            script("postinst", "#!/bin/sh\nset -e\n"),
            script("postinst", "#!/bin/sh\n# grep bad /etc/passwd\n"),
            script("postinst", "#!/bin/sh\ngetent passwd good || true\n"),
            script(
                "postinst",
                "#!/bin/sh\ngetent passwd good || true  # grep /etc/passwd false-positive\n",
            ),
            script("postinst", "#!/usr/bin/perl\ngrep bad /etc/passwd\n"),
            script("postinst", "#!/bin/sh\ngrep bad /etc/password\n"),
            script("triggers", "#!/bin/sh\ngrep bad /etc/passwd\n"),
        ];
        let fail = vec![
            script("postinst", "#!/bin/sh\ngrep bad /etc/passwd\n"),
            script("postinst", "#!/bin/sh\ngrep bad /etc/group\n"),
            script("postinst", "#!/bin/sh\ngrep -E bad /etc/passwd || true\n"),
            script("postinst", "#!/bin/sh\ngrep -F bad /etc/group || true\n"),
            script("postinst", "#!/bin/sh\negrep bad /etc/passwd\n"),
            script("postinst", "#!/bin/sh\nfgrep bad /etc/group\n"),
            script("postinst", "#!/bin/dash\ngrep bad /etc/passwd\n"),
            script("postinst", "#!/bin/sh\ngrep \\\nbad /etc/passwd\n"),
        ];
        BinaryPackageTester::new(MaintainerScriptShouldNotParseEtcPasswdOrGroup, pass, fail)
            .test_and_snapshot();
    }
}
