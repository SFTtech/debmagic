use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::interpreter::Hashbang;
use crate::lint::rule::SourceRule;

pub struct DebianRulesNotAMakefile;

declare_rule! {
    /// `debian/rules` does not start with `#!/usr/bin/make -f`.
    ///
    /// Follows `debian/rules` through symbolic links (up to eight hops; a
    /// link that is absolute, dangling, or leads to a directory ends the
    /// check, as lintian's `follow` does) and reads the first line of the
    /// file after `#!`, trimmed. The line must be `/usr/bin/make`, then
    /// whitespace, then `-f`, `-rf`, `-fr`, `-ef`, `-fe`, or `-rfe`. Empty
    /// content, a file without `#!`, and `#!#!#!` count as no hashbang.
    /// Extra is empty. Location is `debian/rules` with no line. Applies to the
    /// Source tree and Source package.
    DebianRulesNotAMakefile,
    code = "LN0911",
    tag = "debian-rules-not-a-makefile",
    default_selected = true,
    experimental = false,
}

static MAKE_HASHBANG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^/usr/bin/make\s+-[re]?f[re]?$").expect("valid regex"));

impl SourceRule for DebianRulesNotAMakefile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(entry) = follow(files, "debian/rules") else {
            return;
        };
        let Ok(bytes) = files.read(entry) else {
            return;
        };
        if MAKE_HASHBANG.is_match(&hashbang(&bytes)) {
            return;
        }
        ctx.diagnostic(Diagnostic::error("").with_location(Location {
            path: PathBuf::from("debian/rules"),
            line: None,
            column: None,
        }));
    }
}

/// The entry at `path` after following symbolic links, or `None` when the
/// chain ends at a directory, an absolute or missing link, or more than eight
/// hops.
fn follow<'a>(files: &'a FileTree, path: &str) -> Option<&'a Entry> {
    let mut path = PathBuf::from(path);
    for _ in 0..8 {
        let entry = files.get(&path)?;
        match entry.kind() {
            EntryKind::Symlink => path = resolve_link(&path, entry.link_target()?)?,
            EntryKind::Directory => return None,
            EntryKind::RegularFile | EntryKind::Hardlink => return Some(entry),
        }
    }
    None
}

/// A relative link resolved against the directory of `from`.
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

/// Lintian's `hashbang`: the text after `#!` on the first line, trimmed, or
/// empty when the content is not a script. [`Hashbang`] keeps this line
/// private, so it is read again here.
fn hashbang(bytes: &[u8]) -> String {
    if Hashbang::read(bytes).is_none() {
        return String::new();
    }
    let rest = &bytes[2..];
    let end = rest
        .iter()
        .position(|&byte| byte == b'\n')
        .unwrap_or(rest.len());
    String::from_utf8_lossy(&rest[..end]).trim().to_string()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn rules(content: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/rules", TestFile::new(content))]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            rules("#!/usr/bin/make -f\n%:\n\tdh $@\n"),
            rules("#!/usr/bin/make -rf\n"),
            rules("#!/usr/bin/make -fr\n"),
            rules("#!/usr/bin/make -ef\n"),
            // Trailing whitespace is trimmed.
            rules("#!/usr/bin/make -f   \n"),
            // The link resolves to a valid makefile.
            vec![
                ("debian/rules", TestFile::symlink("real")),
                ("debian/real", TestFile::new("#!/usr/bin/make -f\n")),
            ],
            // A dangling link ends the check: lintian reports nothing here.
            vec![("debian/rules", TestFile::symlink("missing"))],
        ];
        let fail = vec![
            rules("#!/bin/sh\n"),
            rules(""),
            rules("#!/usr/bin/make\n"),
            rules("#!/usr/bin/make -x\n"),
            rules("#!/usr/bin/make -f -x\n"),
            rules("#!/usr/bin/env make -f\n"),
            // No `#!` line at all.
            rules("# not a hashbang\n/usr/bin/make -f\n"),
            // The link resolves to a non-makefile.
            vec![
                ("debian/rules", TestFile::symlink("real")),
                ("debian/real", TestFile::new("#!/bin/sh\n")),
            ],
        ];
        SourceTreeTester::new(DebianRulesNotAMakefile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules("#!/usr/bin/make -f\n"))];
        let fail = vec![SourcePackageCase::from(rules("#!/bin/sh\n"))];
        SourcePackageTester::new(DebianRulesNotAMakefile, pass, fail).test_and_snapshot();
    }
}
