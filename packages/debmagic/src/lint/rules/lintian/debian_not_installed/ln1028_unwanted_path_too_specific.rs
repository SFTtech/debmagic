use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::SourceRule;

pub struct UnwantedPathTooSpecific;

declare_rule! {
    /// A line of `debian/not-installed` names a path with an architecture
    /// triplet under `usr/lib/`, such as `usr/lib/x86_64-linux-gnu/foo.a`.
    ///
    /// Lintian's warning, one Diagnostic per such line, with the line (without
    /// its line break) as Extra and `debian/not-installed:<line>` as the
    /// Location. The line must start with `usr/lib/` followed by a segment of
    /// the form `<a>-<b>-<c>/`; a `usr/lib/*/` wildcard is not reported. Comment
    /// lines (leading whitespace, then `#`) are skipped. The file must be a
    /// regular file, a hardlink, or a symlink that resolves to one inside the
    /// tree; otherwise nothing is reported. Lines are matched as bytes, so a
    /// non-UTF-8 line is still reported, with its extra decoded lossily.
    UnwantedPathTooSpecific,
    code = "LN1028",
    tag = "unwanted-path-too-specific",
    default_selected = true,
    experimental = false,
}

static TRIPLET_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/lib/[^/-]+-[^/-]+-[^/-]+/").expect("valid regex"));

impl SourceRule for UnwantedPathTooSpecific {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let path = PathBuf::from("debian/not-installed");
        let Some(entry) = files.get(&path) else {
            return;
        };
        let Some(entry) = open_entry(files, entry) else {
            return;
        };
        let Ok(bytes) = files.read(entry) else {
            return;
        };
        for (index, line) in bytes.split(|byte| *byte == b'\n').enumerate() {
            if line.trim_ascii_start().starts_with(b"#") {
                continue;
            }
            if !TRIPLET_PATH.is_match(line) {
                continue;
            }
            ctx.diagnostic(
                Diagnostic::warn(String::from_utf8_lossy(line).into_owned()).with_location(
                    Location {
                        path: path.clone(),
                        line: Some(index as u32 + 1),
                        column: None,
                    },
                ),
            );
        }
    }
}

/// Lintian's `is_open_ok`: the entry itself, or the regular file or hardlink
/// a symlink chain resolves to inside the tree (at most 8 links).
fn open_entry<'t>(files: &'t FileTree, entry: &'t Entry) -> Option<&'t Entry> {
    let mut current = entry;
    for _ in 0..8 {
        match current.kind() {
            EntryKind::RegularFile | EntryKind::Hardlink => return Some(current),
            EntryKind::Symlink => {
                let mut resolved = current.path().parent()?.to_path_buf();
                for component in current.link_target()?.components() {
                    match component {
                        Component::Normal(part) => resolved.push(part),
                        Component::CurDir => {}
                        Component::ParentDir => {
                            if !resolved.pop() {
                                return None;
                            }
                        }
                        Component::RootDir | Component::Prefix(_) => return None,
                    }
                }
                current = files.get(Path::new(&resolved))?;
            }
            _ => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    fn tree(not_installed: &str) -> Vec<(String, TestFile)> {
        vec![
            ("debian/control".to_string(), TestFile::from(CONTROL)),
            (
                "debian/not-installed".to_string(),
                TestFile::from(not_installed),
            ),
        ]
    }

    #[test]
    fn source() {
        let pass = vec![
            vec![("debian/control".to_string(), TestFile::from(CONTROL))],
            tree(""),
            tree("usr/lib/*/does-not-exist.a\n"),
            tree("usr/lib/x86_64/foo\n"),
            tree("usr/lib/x86_64-linux/foo\n"),
            tree("usr/lib/x86_64-linux-gnu\n"),
            tree("# usr/lib/x86_64-linux-gnu/foo\n"),
            tree("  # usr/lib/x86_64-linux-gnu/foo\n"),
            tree("usr/share/x86_64-linux-gnu/foo\n"),
            tree(" usr/lib/x86_64-linux-gnu/foo\n"),
            vec![
                ("debian/control".to_string(), TestFile::from(CONTROL)),
                (
                    "debian/not-installed".to_string(),
                    TestFile::symlink("/usr/lib/x86_64-linux-gnu/foo"),
                ),
            ],
        ];
        let fail = vec![
            tree("usr/lib/x86_64-linux-gnu/does-not-exist.a\n"),
            tree("usr/lib/x86_64-linux-gnu/foo\nusr/lib/a-b-c/bar"),
            tree("usr/lib/x86_64-linux-gnu/foo\n# usr/lib/a-b-c/bar\n\nusr/lib/a-b-c/baz\n"),
            tree("usr/lib/x86_64-linux-gnu/foo\r\n"),
            vec![
                ("debian/control".to_string(), TestFile::from(CONTROL)),
                (
                    "debian/not-installed".to_string(),
                    TestFile::from_bytes(b"usr/lib/x86_64-linux-gnu/\xff\n".to_vec()),
                ),
            ],
            vec![
                ("debian/control".to_string(), TestFile::from(CONTROL)),
                (
                    "debian/not-installed".to_string(),
                    TestFile::symlink("link-target"),
                ),
                (
                    "debian/link-target".to_string(),
                    TestFile::from("usr/lib/x86_64-linux-gnu/foo\n"),
                ),
            ],
        ];
        SourceTreeTester::new(UnwantedPathTooSpecific, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(tree("usr/lib/*/foo\n"))];
        let fail = vec![SourcePackageCase::from(tree(
            "usr/lib/x86_64-linux-gnu/foo\n",
        ))];
        SourcePackageTester::new(UnwantedPathTooSpecific, pass, fail).test_and_snapshot();
    }
}
