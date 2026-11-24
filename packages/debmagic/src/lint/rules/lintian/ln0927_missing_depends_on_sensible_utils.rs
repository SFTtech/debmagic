use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::file_type::FileType;
use crate::lint::interpreter::Hashbang;
use crate::lint::rule::BinaryPackageRule;

pub struct MissingDependsOnSensibleUtils;

declare_rule! {
    /// A shipped file or maintainer script runs a `sensible-utils` command
    /// (`sensible-browser`, `sensible-editor`, `sensible-pager`) but the
    /// Binary package does not depend on `sensible-utils`.
    ///
    /// Skipped when the Binary package's `Pre-Depends`, `Depends`,
    /// `Recommends` or `Suggests` satisfy `sensible-utils`, or when the source
    /// name is `sensible-utils`. Otherwise the first mention in each regular
    /// file or hardlink of the Data files, and in each maintainer script
    /// (`config`, `preinst`, `postinst`, `prerm`, `postrm`), is reported with
    /// the mentioned command as extra and the path as the Location. A mention
    /// is a whole word `select-editor`, `sensible-browser`, `sensible-editor`
    /// or `sensible-pager` not preceded by `-`, found as lintian's
    /// `mentions_in_operation`: in an ELF (outside `usr/lib/debug/`), or in a
    /// script (`#!`, not the lincity magic `#!#!#!`). Only the first mention
    /// in a file is reported, as lintian does.
    MissingDependsOnSensibleUtils,
    code = "LN0927",
    tag = "missing-depends-on-sensible-utils",
    default_selected = true,
    experimental = false,
}

/// The `(?<!-)` lookbehind of lintian's regex is checked by hand, since the
/// `regex` crate has none.
static SENSIBLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:select-editor|sensible-(?:browser|editor|pager))(?-u:\b)").expect("valid regex")
});

const RELATIONS: &[&str] = &["Pre-Depends", "Depends", "Recommends", "Suggests"];
const MAINTAINER_SCRIPTS: &[&str] = &["config", "preinst", "postinst", "prerm", "postrm"];

impl BinaryPackageRule for MissingDependsOnSensibleUtils {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let depends = ctx
            .installation_control()
            .fields()
            .relations_and(RELATIONS)
            .satisfies("sensible-utils");
        if depends || ctx.source_name() == "sensible-utils" {
            return;
        }
        let mut hits = Vec::new();
        if let Some(files) = ctx.data_files() {
            hits.extend(scan(files, false));
        }
        if let Some(files) = ctx.control_files() {
            hits.extend(scan(files, true));
        }
        for (path, command) in hits {
            ctx.diagnostic(Diagnostic::error(command).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// The regular files and hardlinks of a tree that mention a `sensible-utils`
/// command, with the first command each one mentions. Control files are
/// limited to the maintainer scripts.
fn scan(files: &FileTree, maintainer_only: bool) -> Vec<(PathBuf, String)> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .filter(|entry| !maintainer_only || is_maintainer_script(entry))
        .filter_map(|entry| {
            let bytes = files.read(entry).ok()?;
            let command = mentions_in_operation(entry, &bytes)?;
            Some((entry.path().to_path_buf(), command))
        })
        .collect()
}

fn is_maintainer_script(entry: &Entry) -> bool {
    entry
        .path()
        .to_str()
        .is_some_and(|name| MAINTAINER_SCRIPTS.contains(&name))
}

/// `mentions_in_operation`: the first command in an ELF's strings (outside
/// `usr/lib/debug/`), else in a script's bytes.
fn mentions_in_operation(entry: &Entry, bytes: &[u8]) -> Option<String> {
    if is_elf(entry, bytes)
        && !is_debug_path(entry.path())
        && let Some(command) = first_mention(bytes)
    {
        return Some(command);
    }
    if Hashbang::is_script(bytes) {
        return first_mention(bytes);
    }
    None
}

fn first_mention(bytes: &[u8]) -> Option<String> {
    SENSIBLE.find_iter(bytes).find_map(|found| {
        let preceded_by_dash = found.start() > 0 && bytes[found.start() - 1] == b'-';
        (!preceded_by_dash).then(|| String::from_utf8_lossy(found.as_bytes()).into_owned())
    })
}

fn is_elf(entry: &Entry, bytes: &[u8]) -> bool {
    matches!(entry.file_type(), Some(FileType::Elf { .. })) || bytes.starts_with(b"\x7fELF")
}

fn is_debug_path(path: &Path) -> bool {
    path.to_str()
        .is_some_and(|name| name.starts_with("usr/lib/debug/"))
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
            // Each relation field satisfies sensible-utils.
            data("usr/bin/tool", script("sensible-editor\n")).field("Depends", "sensible-utils"),
            data("usr/bin/tool", script("sensible-editor\n"))
                .field("Pre-Depends", "sensible-utils"),
            data("usr/bin/tool", script("sensible-editor\n"))
                .field("Recommends", "sensible-utils (>= 1.0)"),
            data("usr/bin/tool", script("sensible-editor\n")).field("Suggests", "sensible-utils"),
            // The source name is sensible-utils itself.
            data("usr/bin/tool", script("sensible-editor\n")).field("Source", "sensible-utils"),
            // Plain text is not an operation.
            data(
                "usr/share/doc/example/README",
                TestFile::new("sensible-editor\n"),
            ),
            // A hyphen before the command is not a mention.
            data("usr/bin/tool", script("my-sensible-editor\n")),
            data("usr/bin/tool", script("sensible-editorx\n")),
            // Debug ELF has no collected strings.
            data("usr/lib/debug/usr/bin/test", elf("sensible-editor\n")),
            // Not a maintainer script.
            maintainer("md5sums", TestFile::new("#!/bin/sh\nsensible-editor\n")),
            // No shebang: not a script.
            maintainer("postinst", TestFile::new("sensible-editor\n")),
            // Lincity data magic is not a script.
            data(
                "usr/share/games/lincity",
                TestFile::new("#!#!#!\nsensible-editor\n"),
            ),
            data("usr/bin/tool", TestFile::symlink("sensible-editor")),
        ];
        let fail = vec![
            data("usr/bin/test", script("sensible-editor\n")),
            data("usr/bin/test", script("sensible-pager\n")).udeb(),
            data("usr/bin/test", elf("sensible-browser\n")),
            // Only the first mention is reported.
            data("usr/bin/test", script("select-editor\nsensible-editor\n")),
            // A mention in a hardlink counts.
            data("usr/bin/test", script("sensible-editor\n").hardlink()),
            maintainer("postinst", script("sensible-pager\n")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("preinst", script("sensible-editor\n"))
                .control_file("postinst", script("select-editor\n")),
            // A relation that is not satisfied still reports, and Conflicts is not read.
            data("usr/bin/test", script("sensible-editor\n")).field("Depends", "foo"),
            // An alternative does not imply the package.
            data("usr/bin/test", script("sensible-editor\n"))
                .field("Depends", "foo | sensible-utils"),
            data("usr/bin/test", script("sensible-editor\n")).field("Conflicts", "sensible-utils"),
        ];
        BinaryPackageTester::new(MissingDependsOnSensibleUtils, pass, fail).test_and_snapshot();
    }
}
