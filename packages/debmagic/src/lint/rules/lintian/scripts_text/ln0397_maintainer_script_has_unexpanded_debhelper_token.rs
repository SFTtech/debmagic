use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::BinaryPackageRule;

use super::scan::{script_entries, shebang_token};

pub struct MaintainerScriptHasUnexpandedDebhelperToken;

declare_rule! {
    /// A maintainer script still contains `#DEBHELPER#`.
    ///
    /// Control files named `config…` or ending in `preinst`, `postinst`,
    /// `prerm`, or `postrm` are scanned when the shebang names an interpreter.
    /// Each physical line containing the token is reported. Extra is
    /// `#DEBHELPER#`. The Location is the script and that line. Udebs are
    /// checked: the substitution skip for udebs is not this Tag.
    MaintainerScriptHasUnexpandedDebhelperToken,
    code = "LN0397",
    tag = "maintainer-script-has-unexpanded-debhelper-token",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MaintainerScriptHasUnexpandedDebhelperToken {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        for (path, line) in tokens(files) {
            ctx.diagnostic(Diagnostic::warn("#DEBHELPER#").with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

fn tokens(files: &FileTree) -> Vec<(PathBuf, u32)> {
    let mut found = Vec::new();
    for (entry, bytes) in script_entries(files) {
        if shebang_token(&bytes).is_none() {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        for (index, line) in physical_lines(&text).into_iter().enumerate() {
            if line.contains("#DEBHELPER#") {
                found.push((
                    entry.path().to_path_buf(),
                    u32::try_from(index + 1).unwrap_or(u32::MAX),
                ));
            }
        }
    }
    found
}

fn physical_lines(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines
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
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            script("postinst", "#!/bin/sh\nset -e\n"),
            script("preinst", "#DEBHELPER#\n"),
            script("postinst", "#!/bin/sh\n# debhelper already expanded\n"),
            script("config", "#!/bin/sh\necho ok\n"),
        ];
        let fail = vec![
            script("postinst", "#!/bin/sh\nset -e\n#DEBHELPER#\n"),
            script(
                "preinst",
                "#!/bin/sh\n#\n#\n#\n#\n#\n#\n#\n#\n#\n#\n#\n#DEBHELPER#\n",
            ),
            script("prerm", "#!/bin/sh\n#DEBHELPER#\necho still\n#DEBHELPER#\n"),
            script("config", "#!/bin/sh\n#DEBHELPER#\n").udeb(),
        ];
        BinaryPackageTester::new(MaintainerScriptHasUnexpandedDebhelperToken, pass, fail)
            .test_and_snapshot();
    }
}
