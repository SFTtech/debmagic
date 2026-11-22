use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::interpreter::Hashbang;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct OldStyleConfigScript;

declare_rule! {
    /// An old style `*-config` script in `usr/bin/` that a `pkg-config` file
    /// would replace.
    ///
    /// Walks regular files (not hardlinks) under `usr/bin/` whose name ends in
    /// `-config` and that start with a `#!` line (lincity `#!#!#!` excluded).
    /// The first 8192 bytes must contain `flag`, `/include/` or `pkg-config`.
    /// The architecture specific variant is `old-style-config-script-multiarch-path`
    /// and its `-arch-all` sibling. Extra is empty; the path is the Location.
    /// Severity is pedantic and the Tag is not selected by default, as in
    /// lintian.
    OldStyleConfigScript,
    code = "LN1380",
    tag = "old-style-config-script",
    default_selected = false,
    experimental = false,
}

/// The first window lintian reads of a file.
const WINDOW: usize = 8192;

impl BinaryPackageRule for OldStyleConfigScript {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<PathBuf> = files
            .query()
            .kinds([EntryKind::RegularFile])
            .into_iter()
            .filter(|entry| is_old_style_config_script(files, entry))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in hits {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

fn is_old_style_config_script(files: &FileTree, entry: &Entry) -> bool {
    let Some(name) = entry.path().to_str() else {
        return false;
    };
    if !name.starts_with("usr/bin/") || !name.ends_with("-config") {
        return false;
    }
    let Ok(content) = files.read(entry) else {
        return false;
    };
    if Hashbang::read(&content).is_none() {
        return false;
    }
    let block = &content[..content.len().min(WINDOW)];
    contains(block, b"flag") || contains(block, b"/include/") || contains(block, b"pkg-config")
}

fn contains(block: &[u8], needle: &[u8]) -> bool {
    block.windows(needle.len()).any(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(body: &str) -> TestFile {
        TestFile::new(format!("#!/bin/sh\n{body}"))
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(vec![("usr/bin/foo-config", script("echo 1.0\n"))]),
            BinaryPackageCase::from(vec![("usr/bin/foo", script("flag=1\n"))]),
            // Not a script.
            BinaryPackageCase::from(vec![(
                "usr/bin/foo-config",
                TestFile::new("flag pkg-config /include/\n"),
            )]),
            // Only the first 8192 bytes are read.
            BinaryPackageCase::from(vec![(
                "usr/bin/foo-config",
                TestFile::new(format!("#!/bin/sh\n{}flag\n", "x".repeat(8192))),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/bin/foo-config",
                script("echo --cflags\n# flag\n"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/bin/foo-config",
                script("echo -I/usr/include/foo\n"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/bin/foo-config",
                script("pkg-config --libs foo\n"),
            )]),
        ];
        BinaryPackageTester::new(OldStyleConfigScript, pass, fail).test_and_snapshot();
    }
}
