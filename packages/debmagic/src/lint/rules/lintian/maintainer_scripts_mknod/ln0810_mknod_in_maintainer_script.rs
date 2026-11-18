use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::maintainer_script::{Continuations, maintainer_script_lines};
use crate::lint::rule::BinaryPackageRule;

pub struct MknodInMaintainerScript;

declare_rule! {
    /// A maintainer script calls `mknod` to create a device file.
    ///
    /// Logical lines of every maintainer script with an interpreter, after
    /// comment removal and `\` joins. A line is flagged when it starts with
    /// `mknod` (after whitespace, followed by whitespace or the end) and does
    /// not contain whitespace, `p`, whitespace, so `mknod foo p` at the end of a
    /// line is flagged. Extra is empty. Location is the last physical line of
    /// the logical line. Udebs are checked too.
    MknodInMaintainerScript,
    code = "LN0810",
    tag = "mknod-in-maintainer-script",
    default_selected = true,
    experimental = false,
}

static MKNOD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*mknod(?:\s|$)").expect("valid regex"));
static FIFO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\sp\s").expect("valid regex"));

impl BinaryPackageRule for MknodInMaintainerScript {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        let hits: Vec<_> = maintainer_script_lines(files, Continuations::Always)
            .into_iter()
            .filter(|line| MKNOD.is_match(&line.text) && !FIFO.is_match(&line.text))
            .map(|line| (line.path, line.number))
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

    fn with_script(name: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(content))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            with_script("postinst", "#!/bin/sh\nmknod /tmp/x p 0 0\n"),
            with_script("postinst", "#!/bin/sh\n# mknod /dev/x c 1 3\n"),
            with_script("postinst", "#!/bin/sh\necho mknod\n"),
            with_script("postinst", "#!/bin/sh\nmknodfoo\n"),
            with_script("postinst", "#!/bin/sh\necho x; mknod /dev/x c 1 3\n"),
            with_script("postinst", "mknod /dev/x c 1 3\n"),
            with_script("triggers", "#!/bin/sh\nmknod /dev/x c 1 3\n"),
            BinaryPackageCase::from(vec![(
                "usr/bin/example",
                TestFile::new("#!/bin/sh\nmknod /dev/x c 1 3\n"),
            )]),
        ];
        let fail = vec![
            with_script("postinst", "#!/bin/sh\nmknod /dev/x c 1 3\n"),
            with_script("postinst", "#!/bin/sh\nset -e\n  mknod /dev/x c 1 3\n"),
            with_script("postinst", "#!/bin/sh\nmknod /dev/x \\\n c 1 3\n"),
            with_script("postinst", "#!/bin/sh\nmknod foo p\n"),
            with_script("postinst", "#!/bin/sh\nmknod /dev/x c 1 3\n").udeb(),
        ];
        BinaryPackageTester::new(MknodInMaintainerScript, pass, fail).test_and_snapshot();
    }
}
