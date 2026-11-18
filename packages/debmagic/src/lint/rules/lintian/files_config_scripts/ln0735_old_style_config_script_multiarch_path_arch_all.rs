use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::interpreter::Hashbang;
use crate::lint::rule::BinaryPackageRule;

pub struct OldStyleConfigScriptMultiarchPathArchAll;

declare_rule! {
    /// An old style config script in an `Architecture: all` package names a
    /// DEB_HOST_MULTIARCH triplet.
    ///
    /// Only when Architecture is exactly `all`. Walks regular files (not
    /// hardlinks) under `usr/bin/` whose name ends in `-config` and that start
    /// with a `#!` line (lincity `#!#!#!` excluded). The first 8192 bytes must
    /// contain `flag`, `/include/` or `pkg-config`. Then the first triplet from
    /// the vendored `architectures/host.json`, in sorted order, that appears in
    /// those bytes with a non-word byte on each side is reported. A triplet
    /// that begins the file's basename is skipped. The message is
    /// `full text contains architecture specific dir <triplet>`; the path is
    /// the Location.
    OldStyleConfigScriptMultiarchPathArchAll,
    code = "LN0735",
    tag = "old-style-config-script-multiarch-path-arch-all",
    default_selected = true,
    experimental = false,
}

/// The first window lintian reads of a file.
const WINDOW: usize = 8192;

impl BinaryPackageRule for OldStyleConfigScriptMultiarchPathArchAll {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.installation_control().fields().get("Architecture") != Some("all") {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<(PathBuf, String)> = files
            .query()
            .kinds([EntryKind::RegularFile])
            .into_iter()
            .filter_map(|entry| {
                triplet_in_config_script(files, entry)
                    .map(|triplet| (entry.path().to_path_buf(), triplet))
            })
            .collect();
        for (path, triplet) in hits {
            ctx.diagnostic(
                Diagnostic::error(format!(
                    "full text contains architecture specific dir {triplet}"
                ))
                .with_location(Location {
                    path,
                    line: None,
                    column: None,
                }),
            );
        }
    }
}

/// The triplet an old style config script names, if any.
fn triplet_in_config_script(files: &FileTree, entry: &Entry) -> Option<String> {
    let name = entry.path().to_str()?;
    if !name.starts_with("usr/bin/") || !name.ends_with("-config") {
        return None;
    }
    let content = files.read(entry).ok()?;
    Hashbang::read(&content)?;
    let block = &content[..content.len().min(WINDOW)];
    let config =
        contains(block, b"flag") || contains(block, b"/include/") || contains(block, b"pkg-config");
    if !config {
        return None;
    }
    let basename = entry.path().file_name()?.to_str()?;
    TRIPLETS
        .iter()
        .find(|triplet| {
            mentions(block, triplet.as_bytes()) && !basename.starts_with(triplet.as_str())
        })
        .cloned()
}

fn contains(block: &[u8], needle: &[u8]) -> bool {
    block.windows(needle.len()).any(|window| window == needle)
}

/// `\W<needle>(\W|$)`: the needle with a non-word byte before and after it.
/// A needle at the start of the block has no `\W` before it and does not match.
fn mentions(block: &[u8], needle: &[u8]) -> bool {
    block
        .windows(needle.len())
        .enumerate()
        .any(|(index, window)| {
            window == needle
                && index > 0
                && !is_word(block[index - 1])
                && block
                    .get(index + needle.len())
                    .is_none_or(|&byte| !is_word(byte))
        })
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Every DEB_HOST_MULTIARCH from the vendored `host.json`, sorted and unique.
static TRIPLETS: LazyLock<Vec<String>> = LazyLock::new(|| {
    let file: serde_json::Value =
        serde_json::from_str(include_str!("../../../data/architectures/host.json"))
            .expect("vendored architectures/host.json");
    file["host_variables"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(_, host)| Some(host["DEB_HOST_MULTIARCH"].as_str()?.to_string()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
});

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(body: &str) -> TestFile {
        TestFile::new(format!("#!/bin/sh\n{body}"))
    }

    fn all(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files).field("Architecture", "all")
    }

    #[test]
    fn needle_boundaries() {
        assert!(mentions(
            b"-I/usr/include/x86_64-linux-gnu/",
            b"x86_64-linux-gnu"
        ));
        assert!(mentions(b" x86_64-linux-gnu\n", b"x86_64-linux-gnu"));
        assert!(!mentions(b"x86_64-linux-gnu", b"x86_64-linux-gnu"));
        assert!(!mentions(b"foo_x86_64-linux-gnu", b"x86_64-linux-gnu"));
        assert!(!mentions(b"x86_64-linux-gnu2", b"x86_64-linux-gnu"));
    }

    #[test]
    fn config_markers_are_plain_substrings() {
        assert!(contains(b"flagged", b"flag"));
        assert!(contains(b"usr/include/", b"/include/"));
        assert!(!contains(b"usr/include", b"/include/"));
    }

    #[test]
    fn test() {
        let beyond_window = format!("#!/bin/sh\nflag\n{}x86_64-linux-gnu", " ".repeat(WINDOW));
        let pass = vec![
            BinaryPackageCase::from(vec![(
                "usr/bin/arch-all-config",
                script("echo -I/usr/include/x86_64-linux-gnu\n"),
            )])
            .field("Architecture", "amd64"),
            all(vec![(
                "usr/bin/arch-all-config",
                script("echo -I/usr/include/arch\n"),
            )]),
            all(vec![(
                "usr/lib/arch-all-config",
                script("flag x86_64-linux-gnu\n"),
            )]),
            all(vec![(
                "usr/bin/arch-all-tool",
                script("flag x86_64-linux-gnu\n"),
            )]),
            // Not a script.
            all(vec![(
                "usr/bin/arch-all-config",
                TestFile::new("flag x86_64-linux-gnu\n"),
            )]),
            // No config marker in the text.
            all(vec![(
                "usr/bin/arch-all-config",
                script("echo x86_64-linux-gnu\n"),
            )]),
            // No boundary before the triplet.
            all(vec![(
                "usr/bin/arch-all-config",
                script("flag foo_x86_64-linux-gnu\n"),
            )]),
            // The basename starts with the triplet.
            all(vec![(
                "usr/bin/x86_64-linux-gnu-foo-config",
                script("flag x86_64-linux-gnu\n"),
            )]),
            all(vec![(
                "usr/bin/i386-linux-gnu-config",
                script("flag i386-linux-gnu\n"),
            )]),
            // Only past the first window.
            all(vec![(
                "usr/bin/arch-all-config",
                TestFile::from_bytes(beyond_window.into_bytes()),
            )]),
            all(vec![(
                "usr/bin/arch-all-config",
                TestFile::symlink("other-config"),
            )]),
            all(vec![(
                "usr/bin/arch-all-config",
                TestFile::new("#!#!#!\nflag x86_64-linux-gnu\n"),
            )]),
        ];
        let fail = vec![
            all(vec![(
                "usr/bin/arch-all-config",
                script("echo -I/usr/include/x86_64-linux-gnu/foo\n"),
            )]),
            all(vec![(
                "usr/bin/arch-all-config",
                script("# pkg-config\necho x86_64-linux-gnu"),
            )]),
            all(vec![(
                "usr/bin/arch-all-config",
                script("flag i386-linux-gnu\n"),
            )]),
            all(vec![(
                "usr/bin/arch-all-config",
                script("flag x86_64-linux-gnu\n"),
            )])
            .field("Multi-Arch", "foreign"),
            all(vec![(
                "usr/bin/arch-all-config",
                script("flag x86_64-linux-gnu\n"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(OldStyleConfigScriptMultiarchPathArchAll, pass, fail)
            .test_and_snapshot();
    }
}
