use std::path::{Component, Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::SourceRule;

pub struct ExecutableDebhelperFileWithoutBeingExecutable;

declare_rule! {
    /// A debhelper filename config under `debian/` is executable but has no
    /// hashbang.
    ///
    /// Only at compat level 9 and above, with a dh command in `debian/rules`.
    /// Walks the direct children of `debian/` (symlinks and regular files or
    /// hardlinks, followed through relative symlinks inside the tree) whose
    /// text after the last dot is a debhelper filename config (`dirs`, `docs`,
    /// `examples`, `info`, `install`, `links`, `manpages`, `mime`,
    /// `sgmlcatalogs`, `sharedmimeinfo`, `wm`). A hit is an executable file
    /// whose hashbang (the text after `#!`, trimmed) is empty or absent: a file
    /// without `#!`, a bare `#!`, or one starting with `#!#!#!`. Extra is empty;
    /// the `debian/` path is the Location. Error severity, from the `.tag` file.
    ExecutableDebhelperFileWithoutBeingExecutable,
    code = "LN0863",
    tag = "executable-debhelper-file-without-being-executable",
    default_selected = true,
    experimental = false,
}

/// Compat level from which debhelper honours executable filename configs.
const USES_EXECUTABLE_FILES: u32 = 9;

/// Debhelper filename configs (lintian `data/debhelper/filename-config-files`,
/// 2.144.0). Same list as the sibling `rules_and_control` Rules.
const FILENAME_CONFIGS: &[&str] = &[
    "dirs",
    "docs",
    "examples",
    "info",
    "install",
    "links",
    "manpages",
    "mime",
    "sgmlcatalogs",
    "sharedmimeinfo",
    "wm",
];

impl SourceRule for ExecutableDebhelperFileWithoutBeingExecutable {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = ctx.debhelper() else {
            return;
        };
        if !facts.uses_debhelper() || facts.level() < USES_EXECUTABLE_FILES {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        let mut hits: Vec<PathBuf> = Vec::new();
        for entry in files.entries() {
            let path = entry.path();
            if path.parent() != Some(Path::new("debian")) {
                continue;
            }
            if !matches!(
                entry.kind(),
                EntryKind::RegularFile | EntryKind::Hardlink | EntryKind::Symlink
            ) {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !is_filename_config(name) {
                continue;
            }
            let Some(actual) = resolve(files, path) else {
                continue;
            };
            if !is_executable(actual) {
                continue;
            }
            let Ok(bytes) = files.read(actual) else {
                continue;
            };
            if hashbang(&bytes).is_none_or(|line| line.is_empty()) {
                hits.push(path.to_path_buf());
            }
        }
        for path in hits {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// Perl `s/^.+\.//` then membership in the filename configs.
fn is_filename_config(name: &str) -> bool {
    let category = match name.rsplit_once('.') {
        Some((prefix, suffix)) if !prefix.is_empty() => suffix,
        _ => name,
    };
    FILENAME_CONFIGS.contains(&category)
}

/// lintian's `is_executable` on the resolved item: any execute bit.
fn is_executable(entry: &Entry) -> bool {
    entry.kind() != EntryKind::Directory && entry.mode() & 0o111 != 0
}

/// lintian's `hashbang`: the text after `#!` on the first line, trimmed. `None`
/// when the content does not start with `#!`, or starts with `#!#!#!`.
fn hashbang(bytes: &[u8]) -> Option<String> {
    if !bytes.starts_with(b"#!") || bytes.starts_with(b"#!#!#!") {
        return None;
    }
    let rest = &bytes[2..];
    let end = rest
        .iter()
        .position(|&byte| byte == b'\n')
        .unwrap_or(rest.len());
    Some(String::from_utf8_lossy(&rest[..end]).trim().to_string())
}

/// The entry `path` names after following relative symlinks that stay inside
/// the tree (lintian's `resolve_path`). `None` when dangling, absolute,
/// escaping, or looping.
fn resolve<'t>(files: &'t FileTree, path: &Path) -> Option<&'t Entry> {
    let mut current = files.get(path)?;
    for _ in 0..8 {
        if current.kind() != EntryKind::Symlink {
            return Some(current);
        }
        let target = resolve_link(current.path(), current.link_target()?)?;
        current = files.get(&target)?;
    }
    None
}

fn resolve_link(from: &Path, target: &Path) -> Option<PathBuf> {
    if target.is_absolute() {
        return None;
    }
    let mut resolved = from.parent().unwrap_or(Path::new("")).to_path_buf();
    for component in target.components() {
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
    Some(resolved)
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

    const DH_RULES: &str = "#!/usr/bin/make -f\n%:\n\tdh $@\n";

    fn tree(extra: &[(&str, TestFile)]) -> Vec<(String, TestFile)> {
        let mut files = vec![
            ("debian/control".to_string(), TestFile::from(CONTROL)),
            ("debian/rules".to_string(), TestFile::from(DH_RULES)),
            ("debian/compat".to_string(), TestFile::from("9\n")),
        ];
        files.extend(
            extra
                .iter()
                .map(|(path, file)| (path.to_string(), file.clone())),
        );
        files
    }

    fn exec(text: &str) -> TestFile {
        TestFile::new(text).mode(0o755)
    }

    #[test]
    fn source() {
        let pass = vec![
            tree(&[]),
            // Not executable: no hashbang is needed.
            tree(&[("debian/manpages", TestFile::new("doc/foo.1\n"))]),
            // Executable with a hashbang.
            tree(&[("debian/manpages", exec("#!/bin/sh\necho\n"))]),
            tree(&[("debian/install", exec("#!/usr/lib/dh-exec/install\n"))]),
            // Not a filename config.
            tree(&[("debian/foo.bak", exec("x"))]),
            // Only the direct children of debian/ count.
            tree(&[("debian/sub/manpages", exec("x"))]),
            // A dangling symlink is not resolved.
            tree(&[("debian/manpages", TestFile::symlink("../missing"))]),
            {
                let mut files = tree(&[("debian/manpages", exec("x"))]);
                files.retain(|(path, _)| path != "debian/compat");
                files.push(("debian/compat".to_string(), TestFile::from("8\n")));
                files
            },
        ];
        let fail = vec![
            tree(&[("debian/manpages", exec("doc/foo.1\n"))]),
            tree(&[("debian/example.install", exec("x"))]),
            // An empty hashbang, or a #!#!#! prefix, is not a hashbang.
            tree(&[("debian/manpages", exec("#!\n"))]),
            tree(&[("debian/manpages", exec("#!#!#!/bin/sh\n"))]),
            // Through a symlink to the executable file.
            tree(&[
                ("debian/helper", exec("x")),
                ("debian/manpages", TestFile::symlink("helper")),
            ]),
        ];
        SourceTreeTester::new(ExecutableDebhelperFileWithoutBeingExecutable, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(tree(&[]))];
        let fail = vec![SourcePackageCase::from(tree(&[(
            "debian/manpages",
            exec("x"),
        )]))];
        SourcePackageTester::new(ExecutableDebhelperFileWithoutBeingExecutable, pass, fail)
            .test_and_snapshot();
    }
}
