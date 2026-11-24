use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct PossibleBashismInMaintainerScript;

declare_rule! {
    /// A Control file whose `/bin/sh` shebang is followed by a bash-only
    /// feature.
    ///
    /// Same patterns and extra as `bash-term-in-posix-shell`, but the file is
    /// any Control file with an `sh` shebang (maintainer scripts and others),
    /// not an installed file. Severity is warning.
    /// Only regular files and hardlinks are read; a symlink is skipped
    /// where lintian would follow one inside the tree.
    PossibleBashismInMaintainerScript,
    code = "LN1431",
    tag = "possible-bashism-in-maintainer-script",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PossibleBashismInMaintainerScript {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        for (path, finding) in control_hits(files) {
            ctx.diagnostic(Diagnostic::warn(finding.extra).with_location(Location {
                path,
                line: Some(finding.line),
                column: None,
            }));
        }
    }
}

/// Hints for every Control file with an `sh` shebang, read before any
/// diagnostic is emitted.
fn control_hits(files: &FileTree) -> Vec<(PathBuf, scan::Finding)> {
    let mut hits = Vec::new();
    for entry in files.entries() {
        if !entry.is_file() {
            continue;
        }
        let Ok(bytes) = files.read(entry) else {
            continue;
        };
        if !scan::is_sh_script(&bytes) {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        for finding in scan::findings(&text) {
            hits.push((entry.path().to_path_buf(), finding));
        }
    }
    hits
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
            control("postinst", "#!/bin/sh\nset -e\necho hello\n"),
            control("postinst", "#!/bin/bash\nsource ./foo\n"),
            control("postinst", "#!/usr/bin/perl\nsource x\n"),
            control("postinst", "#!/bin/sh\n# echo -e is not used\n"),
            control("postinst", "#!/bin/sh\nread -r line\n"),
            control("config", "#!/bin/sh\nexec -l foo\necho $RANDOM\n"),
        ];
        let fail = vec![
            control("postinst", "#!/bin/sh\necho -e foo\n"),
            control("postrm", "#!/bin/sh\nsource ./foo\n"),
            control("preinst", "#!/bin/sh\nfunction foo {\n}\n"),
            control("postinst", "#!/bin/sh\necho $RANDOM\n"),
            control("triggers", "#!/bin/sh\necho $RANDOM\n"),
            control("postinst", "#!/bin/sh\ncat <<<\"x\"\n"),
        ];
        BinaryPackageTester::new(PossibleBashismInMaintainerScript, pass, fail).test_and_snapshot();
    }
}
