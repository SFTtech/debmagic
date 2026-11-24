use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use regex::bytes::Regex as BytesRegex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::file_type::FileType;
use crate::lint::interpreter::Hashbang;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct BinSbinMismatch;

declare_rule! {
    /// A shipped file or maintainer script names a command under the other
    /// `bin` directory: `usr/sbin/foo` where the package ships `usr/bin/foo`,
    /// or `/usr/bin/foo` where it ships `usr/sbin/foo`.
    ///
    /// Each Data files command (a regular file or hardlink under `bin/`,
    /// `usr/bin/`, `sbin/` or `usr/sbin/`) maps the other three
    /// locations of its basename to its own path, as lintian's
    /// `switched_locations`; a later command with the same basename wins. A
    /// hit is one of those locations found by `mentions_in_operation` in an
    /// ELF (outside `usr/lib/debug/`) or a script (`#!`, not the lincity magic
    /// `#!#!#!`), in the Data files regular files and hardlinks and in the
    /// maintainer scripts (`config`, `preinst`, `postinst`, `prerm`,
    /// `postrm`). The location must be preceded by a non-word byte or the start
    /// of the file and not followed by a word byte, `.` or `-`. Extra is
    /// `confused -> correct`; the file is the Location. Severity is info and
    /// the Tag is experimental and not selected by default, as in lintian.
    ///
    /// Differences from lintian: word characters are ASCII only, and an ELF
    /// is searched as its raw bytes rather than its `strings` output (the same
    /// approximation as `missing-depends-on-sensible-utils`).
    BinSbinMismatch,
    code = "LN1560",
    tag = "bin-sbin-mismatch",
    default_selected = false,
    experimental = true,
}

const MAINTAINER_SCRIPTS: &[&str] = &["config", "preinst", "postinst", "prerm", "postrm"];

static COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:usr/)?s?bin/").expect("valid regex"));
static BIN_DIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:usr/)?bin/").expect("valid regex"));

impl BinaryPackageRule for BinSbinMismatch {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(installed) = ctx.data_files() else {
            return;
        };
        let switched = switched_locations(installed);
        if switched.is_empty() {
            return;
        }
        let mut hits = scan(installed, false, &switched);
        if let Some(control) = ctx.control_files() {
            hits.extend(scan(control, true, &switched));
        }
        for (path, extra) in hits {
            ctx.diagnostic(
                Diagnostic::warn(extra)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    })
                    .with_severity(Severity::Info),
            );
        }
    }
}

/// Lintian's `switched_locations`: the other `bin` locations of each command's
/// basename, mapped to the command's own path. Sorted by key.
fn switched_locations(files: &FileTree) -> BTreeMap<String, String> {
    let mut switched = BTreeMap::new();
    let commands = files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .filter(|entry| COMMAND.is_match(&entry.path().to_string_lossy()));
    for command in commands {
        let path = command.path();
        let Some(basename) = path.file_name().map(|name| name.to_string_lossy()) else {
            continue;
        };
        // Lintian's dirname keeps its trailing slash.
        let dirname = path
            .parent()
            .map(|parent| format!("{}/", parent.to_string_lossy()))
            .unwrap_or_default();
        let others: &[&str] = if BIN_DIR.is_match(&dirname) {
            &["sbin", "usr/sbin"]
        } else {
            &["bin", "usr/bin"]
        };
        for other in others {
            switched.insert(
                format!("{other}/{basename}"),
                path.to_string_lossy().into_owned(),
            );
        }
    }
    switched
}

/// The Location and extra of each switched location a regular file or
/// hardlink of the tree mentions. Control files are limited to the
/// maintainer scripts.
fn scan(
    files: &FileTree,
    maintainer_only: bool,
    switched: &BTreeMap<String, String>,
) -> Vec<(PathBuf, String)> {
    let mut hits = Vec::new();
    for entry in files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
    {
        if maintainer_only && !is_maintainer_script(entry) {
            continue;
        }
        let Ok(bytes) = files.read(entry) else {
            continue;
        };
        if !searchable(entry, &bytes) {
            continue;
        }
        for (confused, correct) in switched {
            if mentions(&bytes, confused) {
                hits.push((
                    entry.path().to_path_buf(),
                    format!("{confused} -> {correct}"),
                ));
            }
        }
    }
    hits
}

fn is_maintainer_script(entry: &Entry) -> bool {
    entry
        .path()
        .to_str()
        .is_some_and(|name| MAINTAINER_SCRIPTS.contains(&name))
}

/// `mentions_in_operation` applies to an ELF outside `usr/lib/debug/`, or to
/// a script.
fn searchable(entry: &Entry, bytes: &[u8]) -> bool {
    let elf =
        matches!(entry.file_type(), Some(FileType::Elf { .. })) || bytes.starts_with(b"\x7fELF");
    (elf && !is_debug_path(entry.path())) || Hashbang::is_script(bytes)
}

fn is_debug_path(path: &Path) -> bool {
    path.to_str()
        .is_some_and(|name| name.starts_with("usr/lib/debug/"))
}

/// `qr{ \B / \Q$confused\E (?![\w.-]) }x`. The `\B` is written as a
/// non-boundary, and the lookahead as a consumed byte or the end, which is
/// enough for a yes or no.
fn mentions(bytes: &[u8], confused: &str) -> bool {
    let pattern = format!(r"(?-u:\B)/{}(?-u:[^\w.-]|$)", regex::escape(confused));
    BytesRegex::new(&pattern).is_ok_and(|regex| regex.is_match(bytes))
}

#[cfg(test)]
mod tests {
    use crate::lint::file_type::FileType;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(body: &str) -> TestFile {
        TestFile::new(format!("#!/bin/sh\n{body}"))
    }

    fn elf(body: &str) -> TestFile {
        TestFile::new(body).file_type(FileType::Elf {
            has_static_symbol_table: false,
            executable: true,
        })
    }

    fn with(files: Vec<(&'static str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // The script names the command where it is shipped.
            with(vec![
                ("usr/bin/cmd", TestFile::new("x")),
                ("usr/bin/caller", script("/usr/bin/cmd\n")),
            ]),
            // A command under sbin, called from sbin.
            with(vec![
                ("usr/sbin/cmd", TestFile::new("x")),
                ("usr/sbin/caller", script("/usr/sbin/cmd\n")),
            ]),
            // Only the commands' own directories are switched: a non-command
            // file is not one.
            with(vec![
                ("usr/share/cmd", TestFile::new("x")),
                ("usr/bin/caller", script("/usr/sbin/cmd\n")),
            ]),
            // Plain text is not an operation.
            with(vec![
                ("usr/bin/cmd", TestFile::new("x")),
                (
                    "usr/share/doc/example/README",
                    TestFile::new("/usr/sbin/cmd\n"),
                ),
            ]),
            // A word byte before the slash, or -, . or a word byte after the name.
            with(vec![
                ("usr/bin/cmd", TestFile::new("x")),
                ("usr/bin/caller", script("x/usr/sbin/cmd\n")),
            ]),
            with(vec![
                ("usr/bin/cmd", TestFile::new("x")),
                ("usr/bin/caller", script("/usr/sbin/cmd-extra\n")),
            ]),
            with(vec![
                ("usr/bin/cmd", TestFile::new("x")),
                ("usr/bin/caller", script("/usr/sbin/cmd.1\n")),
            ]),
            with(vec![
                ("usr/bin/cmd", TestFile::new("x")),
                ("usr/bin/caller", script("/usr/sbin/cmdx\n")),
            ]),
            // Debug ELF has no collected strings.
            with(vec![
                ("usr/bin/cmd", TestFile::new("x")),
                ("usr/lib/debug/usr/bin/caller", elf("/usr/sbin/cmd\n")),
            ]),
            // Not a maintainer script, and no shebang.
            with(vec![("usr/bin/cmd", TestFile::new("x"))])
                .control_file("md5sums", script("/usr/sbin/cmd\n")),
            with(vec![("usr/bin/cmd", TestFile::new("x"))])
                .control_file("postinst", TestFile::new("/usr/sbin/cmd\n")),
            // Lincity data magic is not a script.
            with(vec![
                ("usr/bin/cmd", TestFile::new("x")),
                (
                    "usr/share/games/lincity",
                    TestFile::new("#!#!#!\n/usr/sbin/cmd\n"),
                ),
            ]),
            with(vec![
                ("usr/bin/cmd", TestFile::new("x")),
                ("usr/bin/caller", TestFile::symlink("/usr/sbin/cmd")),
            ]),
        ];
        let fail = vec![
            with(vec![
                ("usr/bin/cmd", TestFile::new("x")),
                ("usr/bin/caller", script("/usr/sbin/cmd\n")),
            ]),
            with(vec![
                ("usr/sbin/cmd", TestFile::new("x")),
                ("usr/bin/caller", script("/bin/cmd\n")),
            ]),
            // The `bin` and `usr/bin` locations of a command under sbin.
            with(vec![
                ("usr/sbin/cmd", TestFile::new("x")),
                ("usr/sbin/caller", script("/bin/cmd /usr/bin/cmd\n")),
            ]),
            // A mention in an ELF counts.
            with(vec![
                ("usr/bin/cmd", TestFile::new("x")),
                ("usr/bin/caller", elf("/usr/sbin/cmd\n")),
            ]),
            // A mention in a hardlink counts.
            with(vec![
                ("usr/bin/cmd", TestFile::new("x")),
                ("usr/bin/caller", script("/usr/sbin/cmd\n").hardlink()),
            ]),
            // A maintainer script counts.
            with(vec![("usr/bin/cmd", TestFile::new("x"))])
                .control_file("postinst", script("/usr/sbin/cmd\n")),
            // A command with the same basename in both directories: the last wins.
            with(vec![
                ("usr/bin/cmd", TestFile::new("x")),
                ("usr/sbin/cmd", TestFile::new("x")),
                ("usr/bin/caller", script("/sbin/cmd\n")),
            ]),
        ];
        BinaryPackageTester::new(BinSbinMismatch, pass, fail).test_and_snapshot();
    }
}
