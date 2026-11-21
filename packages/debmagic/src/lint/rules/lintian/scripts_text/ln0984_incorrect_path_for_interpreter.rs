use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::interpreter::Hashbang;
use crate::lint::rule::BinaryPackageRule;

pub struct IncorrectPathForInterpreter;

declare_rule! {
    /// An installed script starts with `#!/usr/bin/env perl`.
    ///
    /// Policy 10.4 says Perl scripts use `/usr/bin/perl` directly. Only an
    /// interpreter `perl` reached through `/usr/bin/env` is reported; `env`
    /// with another path, or another interpreter, is not. Extra is
    /// `/usr/bin/env perl != /usr/bin/perl`; the path is the Location. The
    /// executable bit is not checked. Installed regular files and hardlinks
    /// under `usr/share/doc/` and `usr/src/` are skipped, examples included:
    /// an example is `example-incorrect-path-for-interpreter`, a different
    /// Tag. Udebs are checked too.
    IncorrectPathForInterpreter,
    code = "LN0984",
    tag = "incorrect-path-for-interpreter",
    default_selected = true,
    experimental = false,
}

const EXTRA: &str = "/usr/bin/env perl != /usr/bin/perl";

impl BinaryPackageRule for IncorrectPathForInterpreter {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for path in env_perl(files) {
            ctx.diagnostic(Diagnostic::warn(EXTRA).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Installed scripts whose hashbang calls `perl` through `/usr/bin/env`.
fn env_perl(files: &FileTree) -> Vec<PathBuf> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .filter_map(|entry| {
            let name = entry.path().to_str()?;
            if name.starts_with("usr/share/doc/") || name.starts_with("usr/src/") {
                return None;
            }
            let bytes = files.read(entry).ok()?;
            let hashbang = Hashbang::read(&bytes)?;
            (hashbang.calls_env() && hashbang.interpreter() == "perl")
                .then(|| entry.path().to_path_buf())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(path: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(content).mode(0o755))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            script("usr/bin/tool", "#!/usr/bin/perl\n"),
            script("usr/bin/tool", "#!/bin/sh\n"),
            // `env` with another path, or another interpreter.
            script("usr/bin/tool", "#!/bin/env perl\n"),
            script("usr/bin/tool", "#!/usr/bin/env python3\n"),
            script("usr/bin/tool", "no shebang\n"),
            // Skipped trees.
            script("usr/share/doc/pkg/tool", "#!/usr/bin/env perl\n"),
            script("usr/share/doc/pkg/examples/tool", "#!/usr/bin/env perl\n"),
            script("usr/src/pkg/tool", "#!/usr/bin/env perl\n"),
        ];
        let fail = vec![
            script("usr/bin/tool", "#!/usr/bin/env perl\n"),
            script("usr/bin/tool", "#!/usr/bin/env perl -w\n"),
            // Not executable is still reported.
            BinaryPackageCase::from(vec![(
                "usr/share/foo/tool",
                TestFile::new("#!/usr/bin/env perl\n"),
            )]),
            script("usr/bin/tool", "#!/usr/bin/env perl\n").udeb(),
        ];
        BinaryPackageTester::new(IncorrectPathForInterpreter, pass, fail).test_and_snapshot();
    }
}
