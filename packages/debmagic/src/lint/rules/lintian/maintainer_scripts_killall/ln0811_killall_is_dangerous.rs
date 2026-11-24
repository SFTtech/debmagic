use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::maintainer_script::{Continuations, maintainer_script_lines};
use crate::lint::rule::BinaryPackageRule;

pub struct KillallIsDangerous;

declare_rule! {
    /// A maintainer script calls `killall`.
    ///
    /// Logical lines of every maintainer script with an interpreter, after
    /// comment removal and `\` joins. A line is flagged when it starts with
    /// `killall` after optional whitespace, followed by whitespace or the end.
    /// Extra is empty. Location is the last physical line of the logical line.
    /// Udebs are checked too.
    KillallIsDangerous,
    code = "LN0811",
    tag = "killall-is-dangerous",
    default_selected = true,
    experimental = false,
}

static KILLALL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*killall(?:\s|$)").expect("valid regex"));

impl BinaryPackageRule for KillallIsDangerous {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        let hits: Vec<_> = maintainer_script_lines(files, Continuations::Always)
            .into_iter()
            .filter(|line| KILLALL.is_match(&line.text))
            .map(|line| (line.path, line.number))
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

    fn with_script(name: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(content))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            with_script("postinst", "#!/bin/sh\n# killall foo\n"),
            with_script("postinst", "#!/bin/sh\necho killall foo\n"),
            with_script("postinst", "#!/bin/sh\nkillallfoo\n"),
            with_script("postinst", "#!/bin/sh\necho x; killall foo\n"),
            with_script("postinst", "killall foo\n"),
            with_script("triggers", "#!/bin/sh\nkillall foo\n"),
            BinaryPackageCase::from(vec![(
                "usr/bin/example",
                TestFile::new("#!/bin/sh\nkillall foo\n"),
            )]),
        ];
        let fail = vec![
            with_script("postinst", "#!/bin/sh\nkillall foo\n"),
            with_script("postrm", "#!/bin/sh\nset -e\n  killall -9 foo\n"),
            with_script("postinst", "#!/bin/sh\nkillall \\\n foo\n"),
            with_script("postinst", "#!/bin/sh\nkillall\n"),
            with_script("postinst", "#!/bin/sh\nkillall foo\n").udeb(),
        ];
        BinaryPackageTester::new(KillallIsDangerous, pass, fail).test_and_snapshot();
    }
}
