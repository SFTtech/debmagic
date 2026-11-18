use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::file_type::FileType;
use crate::lint::rule::BinaryPackageRule;

pub struct UsesDpkgDatabaseDirectly;

declare_rule! {
    /// A shipped file or maintainer script uses dpkg's internal database.
    ///
    /// Walks regular files and hardlinks in Data files, and maintainer scripts
    /// in Control files (`config…`, `preinst`, `postinst`, `prerm`, `postrm`).
    /// A hit is lintian's `mentions_in_operation` of `/var/lib/dpkg`: an ELF
    /// is searched as text (this printable needle is what `strings --all`
    /// reports; ELF string tables are not parsed), and a script (`#!`, not the
    /// lincity magic `#!#!#!`) is searched the same way. ELF under
    /// `usr/lib/debug/` is left out, because lintian collects no strings
    /// there. Other files are ignored even when they contain the path.
    ///
    /// Skipped when Section is exactly `debian-installer`, or when the source
    /// name is `base-files` or `dpkg`. The source name is the first word of
    /// Installation control `Source`, or the binary package name when `Source`
    /// is absent — the same fallback lintian uses for an installable. Extra is
    /// empty; the path is the Location.
    UsesDpkgDatabaseDirectly,
    code = "LN0184",
    tag = "uses-dpkg-database-directly",
    default_selected = true,
    experimental = false,
}

const NEEDLE: &[u8] = b"/var/lib/dpkg";

static MAINTAINER_SCRIPT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^config|(?:pre|post)(?:inst|rm)$").expect("valid regex"));

impl BinaryPackageRule for UsesDpkgDatabaseDirectly {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if exempt(ctx) {
            return;
        }
        let mut paths = Vec::new();
        if let Some(files) = ctx.data_files() {
            paths.extend(mentioning(files, false));
        }
        if let Some(files) = ctx.control_files() {
            paths.extend(mentioning(files, true));
        }
        for path in paths {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Section `debian-installer`, or source name `base-files` / `dpkg`.
fn exempt(ctx: &BinaryPackageContext<'_>) -> bool {
    let section = ctx
        .installation_control()
        .fields()
        .unfolded_value("Section")
        .unwrap_or_default();
    section == "debian-installer" || matches!(ctx.source_name().as_str(), "base-files" | "dpkg")
}

fn mentioning(files: &FileTree, maintainer_only: bool) -> Vec<PathBuf> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .filter(|entry| !maintainer_only || is_maintainer_script(entry))
        .filter(|entry| {
            files
                .read(entry)
                .is_ok_and(|bytes| mentions_in_operation(entry, &bytes))
        })
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

fn is_maintainer_script(entry: &Entry) -> bool {
    entry
        .path()
        .to_str()
        .is_some_and(|name| MAINTAINER_SCRIPT.is_match(name))
}

/// `mentions_in_operation(qr{/var/lib/dpkg})`: ELF strings, else script bytes.
fn mentions_in_operation(entry: &Entry, bytes: &[u8]) -> bool {
    if is_elf(entry, bytes) && !is_debug_path(entry.path()) && contains(bytes) {
        return true;
    }
    is_script(bytes) && contains(bytes)
}

fn is_elf(entry: &Entry, bytes: &[u8]) -> bool {
    matches!(entry.file_type(), Some(FileType::Elf { .. })) || bytes.starts_with(b"\x7fELF")
}

fn is_debug_path(path: &Path) -> bool {
    path.to_str()
        .is_some_and(|name| name.starts_with("usr/lib/debug/"))
}

fn is_script(bytes: &[u8]) -> bool {
    bytes.starts_with(b"#!") && !bytes.starts_with(b"#!#!#!")
}

fn contains(bytes: &[u8]) -> bool {
    bytes.windows(NEEDLE.len()).any(|window| window == NEEDLE)
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

    fn data(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, file)])
    }

    fn maintainer(name: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(name, file)
    }

    #[test]
    fn test() {
        let pass = vec![
            data("usr/bin/ok", script("echo hi\n")),
            // Plain text is not an operation, even when it names the database.
            data(
                "usr/share/doc/example/README",
                TestFile::new("This file references /var/lib/dpkg/foo.\n"),
            ),
            data(
                "var/lib/test/misc.txt",
                TestFile::new("false positive /var/lib/dpkg/foo\n"),
            ),
            data("usr/bin/apt", script("echo /var/lib/apt/lists\n")),
            elf_without_needle(),
            // Debug ELF has no collected strings.
            data(
                "usr/lib/debug/usr/bin/test",
                elf("strings /var/lib/dpkg/status\n"),
            ),
            // Not a maintainer script.
            maintainer("md5sums", TestFile::new("#!/bin/sh\necho /var/lib/dpkg\n")),
            // No shebang: not a script, so the bytes are not an operation.
            maintainer("postinst", TestFile::new("echo /var/lib/dpkg/status\n")),
            maintainer("postinst", script("echo ok\n")),
            // Lincity data magic is not a script.
            data(
                "usr/share/games/lincity",
                TestFile::new("#!#!#!\n/var/lib/dpkg\n"),
            ),
            data("usr/bin/tool", TestFile::symlink("elsewhere")),
            data("usr/bin/tool", script("echo /var/lib/dpkg/status\n"))
                .field("Section", "debian-installer"),
            data("usr/bin/tool", script("echo /var/lib/dpkg/status\n")).field("Source", "dpkg"),
            data("usr/bin/tool", script("echo /var/lib/dpkg/status\n")).package("base-files"),
            data("usr/bin/tool", script("echo /var/lib/dpkg/status\n"))
                .field("Source", "dpkg (1.22.11)"),
            data("usr/bin/tool", script("echo /var/lib/dpkg/status\n"))
                .field("Source", "base-files (13.8)"),
        ];
        let fail = vec![
            data("usr/bin/test", script("/bin/grep /var/lib/dpkg/status\n")),
            data("usr/bin/test", elf("printf /var/lib/dpkg\n")),
            data(
                "usr/bin/test",
                script("echo /var/lib/dpkg/info/other.list\n").hardlink(),
            ),
            maintainer("postinst", script("sed /var/lib/dpkg/status\n")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("preinst", script("grep /var/lib/dpkg/status\n"))
                .control_file("postinst", script("echo /var/lib/dpkg/info/pkg.list\n")),
            maintainer("config", script("echo /var/lib/dpkg\n")),
            // A component prefix is not Section debian-installer.
            data("usr/bin/tool", script("echo /var/lib/dpkg\n"))
                .field("Section", "non-free/debian-installer"),
            // The source name is the package name only when Source is absent,
            // and only an exact `dpkg` or `base-files` is exempt.
            data("usr/bin/tool", script("echo /var/lib/dpkg\n")).package("dpkg-dev"),
            // Magic is enough when File type was not stored.
            data(
                "usr/bin/from-magic",
                TestFile::from_bytes(b"\x7fELF /var/lib/dpkg"),
            ),
        ];
        BinaryPackageTester::new(UsesDpkgDatabaseDirectly, pass, fail).test_and_snapshot();
    }

    fn elf_without_needle() -> BinaryPackageCase {
        data("usr/bin/test", elf("printf hello\n"))
    }
}
