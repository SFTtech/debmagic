use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::interpreter::{self, Hashbang};
use crate::lint::rule::BinaryPackageRule;

pub struct InterpreterInUsrLocal;

declare_rule! {
    /// An installed script runs an interpreter under `/usr/local/` that is
    /// not in lintian's interpreter table.
    ///
    /// Debian installs nothing in `/usr/local`. A table name there, such as
    /// `/usr/local/bin/perl`, is `wrong-path-for-interpreter` instead, and is
    /// not reported here. Versioned table names (`python3.7`) count as known.
    /// Extra is the interpreter; the path is the Location. Installed regular
    /// files and hardlinks under `usr/share/doc/`, `usr/src/`, and
    /// `usr/share/cargo/registry/` are skipped. Examples are a different Tag.
    /// Udebs are checked too.
    InterpreterInUsrLocal,
    code = "LN0985",
    tag = "interpreter-in-usr-local",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InterpreterInUsrLocal {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for (path, extra) in in_usr_local(files) {
            ctx.diagnostic(Diagnostic::error(extra).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Installed scripts whose unknown interpreter is under `/usr/local/`.
fn in_usr_local(files: &FileTree) -> Vec<(PathBuf, String)> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .filter_map(|entry| {
            let name = entry.path().to_str()?;
            if name.starts_with("usr/share/doc/")
                || name.starts_with("usr/src/")
                || name.starts_with("usr/share/cargo/registry/")
            {
                return None;
            }
            let bytes = files.read(entry).ok()?;
            let interpreter = Hashbang::read(&bytes)?.interpreter();
            if !interpreter.starts_with("/usr/local/") {
                return None;
            }
            if interpreter::lookup(interpreter::basename(&interpreter)).is_some() {
                return None;
            }
            Some((entry.path().to_path_buf(), interpreter))
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
            script("usr/bin/tool", "#!/usr/bin/special-perl\n"),
            // Table names under /usr/local/ are wrong-path-for-interpreter.
            script("usr/bin/tool", "#!/usr/local/bin/perl\n"),
            script("usr/bin/tool", "#!/usr/local/bin/python3\n"),
            // Not a script.
            BinaryPackageCase::from(vec![(
                "usr/bin/tool",
                TestFile::new("/usr/local/bin/special\n").mode(0o755),
            )]),
            // Skipped trees.
            script("usr/share/doc/pkg/tool", "#!/usr/local/bin/special\n"),
            script(
                "usr/share/doc/pkg/examples/tool",
                "#!/usr/local/bin/special\n",
            ),
            script("usr/src/pkg/tool", "#!/usr/local/bin/special\n"),
            script(
                "usr/share/cargo/registry/pkg/tool",
                "#!/usr/local/bin/special\n",
            ),
        ];
        let fail = vec![
            script("usr/bin/tool", "#!/usr/local/bin/special-perl\n"),
            // `python3.7` is not in the table (lintian's `python` family is absent).
            script("usr/bin/tool", "#!/usr/local/bin/python3.7\n"),
            script("usr/bin/tool", "#!/usr/bin/env /usr/local/bin/special\n"),
            // Not executable is still reported.
            BinaryPackageCase::from(vec![(
                "usr/share/foo/tool",
                TestFile::new("#!/usr/local/bin/special\n"),
            )]),
            script("usr/bin/tool", "#!/usr/local/bin/special\n").udeb(),
        ];
        BinaryPackageTester::new(InterpreterInUsrLocal, pass, fail).test_and_snapshot();
    }
}
