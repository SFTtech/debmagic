use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::interpreter::Hashbang;
use crate::lint::rule::BinaryPackageRule;

pub struct ScriptUsesUnversionedPythonInShebang;

declare_rule! {
    /// An installed script under a `bin` directory runs unversioned `python`.
    ///
    /// Regular Data files whose name contains `bin/` followed by a name with no
    /// further `/` (unanchored, so `usr/share/x/bin/y` counts) and that start
    /// with `#!`. The interpreter is the first word after a leading
    /// `/usr/bin/env`, options dropped. It is flagged when it is `python` or
    /// `/usr/bin/python`. Extra is empty. Location is the installed path. Udebs
    /// are checked too.
    ScriptUsesUnversionedPythonInShebang,
    code = "LN0814",
    tag = "script-uses-unversioned-python-in-shebang",
    default_selected = true,
    experimental = false,
}

static BIN_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:usr/)?bin/[^/]+").expect("valid regex"));
static UNVERSIONED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:/usr/bin/)?python$").expect("valid regex"));

impl BinaryPackageRule for ScriptUsesUnversionedPythonInShebang {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits = unversioned(files);
        for path in hits {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Installed scripts under a `bin` directory whose interpreter is unversioned
/// `python`, in file-tree order.
fn unversioned(files: &FileTree) -> Vec<PathBuf> {
    files
        .query()
        .kinds([EntryKind::RegularFile])
        .into_iter()
        .filter_map(|entry| {
            let name = entry.path().to_str()?;
            if !BIN_NAME.is_match(name) {
                return None;
            }
            let bytes = files.read(entry).ok()?;
            let hashbang = Hashbang::read(&bytes)?;
            UNVERSIONED
                .is_match(&hashbang.interpreter())
                .then(|| entry.path().to_path_buf())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(path: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(content))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            script("usr/bin/script-py3", "#!/usr/bin/python3\n"),
            script("usr/bin/script-py2", "#!/usr/bin/python2\n"),
            script("usr/bin/script-env", "#!/usr/bin/env python3\n"),
            script("usr/bin/script-sh", "#!/bin/sh\n"),
            script("usr/bin/no-shebang", "print()\n"),
            script("usr/lib/foo/script.py", "#!/usr/bin/python\n"),
            script("usr/share/doc/foo/script", "#!/usr/bin/python\n"),
        ];
        let fail = vec![
            script("usr/bin/script-bad1", "#!/usr/bin/python\n"),
            script("usr/bin/script-bad2", "#!/usr/bin/env python\n"),
            script("bin/foo", "#!python\n"),
            script("usr/share/foo/bin/x", "#!/usr/bin/python -u\n"),
            script("usr/bin/script-bad3", "#!/usr/bin/python\n").udeb(),
        ];
        BinaryPackageTester::new(ScriptUsesUnversionedPythonInShebang, pass, fail)
            .test_and_snapshot();
    }
}
