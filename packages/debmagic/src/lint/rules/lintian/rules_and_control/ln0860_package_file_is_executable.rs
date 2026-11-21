use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::SourceRule;

pub struct PackageFileIsExecutable;

declare_rule! {
    /// A packaging file in `debian/` has an executable bit where none belongs.
    ///
    /// Two groups of direct children of `debian/` (symlinks and regular files
    /// or hardlinks, followed through relative symlinks inside the tree) are
    /// reported when the resolved file has any execute bit:
    ///
    /// - `control`, `copyright`, `changelog` and `NEWS`, or `<pkg>.` followed by
    ///   `copyright`, `changelog` or `NEWS`. Always.
    /// - a debhelper filename config (`dirs`, `docs`, `examples`, `info`,
    ///   `install`, `links`, `manpages`, `mime`, `sgmlcatalogs`,
    ///   `sharedmimeinfo`, `wm`, by the text after the last dot) when
    ///   `debian/rules` shows a dh command and the compat level is below 9.
    ///
    /// Extra is empty; the `debian/` path is the Location. Warning severity,
    /// from the `.tag` file. Lintian's `source` walk is gated by the debhelper
    /// facts (no dh command, or a `Build-Driver` other than `debian-rules`,
    /// skips the config group), as is this Rule.
    PackageFileIsExecutable,
    code = "LN0860",
    tag = "package-file-is-executable",
    default_selected = true,
    experimental = false,
}

/// Compat level from which debhelper runs executable filename configs.
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

static PACKAGING_TEXT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:.*\.)?(?:copyright|changelog|NEWS)$").expect("valid regex"));

impl SourceRule for PackageFileIsExecutable {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let debhelper = ctx.debhelper();
        let config_group = debhelper
            .as_ref()
            .is_some_and(|facts| facts.uses_debhelper() && facts.level() < USES_EXECUTABLE_FILES);
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
            let packaging_text = name == "control" || PACKAGING_TEXT.is_match(name);
            if !packaging_text && !(config_group && is_filename_config(name)) {
                continue;
            }
            if resolve(files, path).is_some_and(is_executable) {
                hits.push(path.to_path_buf());
            }
        }
        for path in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
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
    entry.mode() & 0o111 != 0
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
            tree(&[("debian/changelog", TestFile::new("x"))]),
            // debian/compat is not a filename config.
            tree(&[("debian/compat", exec("8\n"))]),
            // Only direct children of debian/ count.
            tree(&[("debian/sub/copyright", exec("x"))]),
            tree(&[("copyright", exec("x"))]),
            // Not a packaging text or filename config name.
            tree(&[("debian/foo.install.bak", exec("x"))]),
            tree(&[("debian/changelog.d", TestFile::directory().mode(0o755))]),
            // A dangling symlink is not resolved.
            tree(&[("debian/copyright", TestFile::symlink("../missing"))]),
            // A symlink to a non-executable file is fine.
            tree(&[
                ("LICENSE", TestFile::new("x")),
                ("debian/copyright", TestFile::symlink("../LICENSE")),
            ]),
        ];
        let fail = vec![
            tree(&[("debian/copyright", exec("x"))]),
            tree(&[("debian/changelog", exec("x"))]),
            tree(&[("debian/NEWS", exec("x"))]),
            tree(&[("debian/example.NEWS", exec("x"))]),
            tree(&[("debian/example.changelog", exec("x"))]),
            // The control file is handled by name too.
            tree(&[("debian/control", exec(CONTROL))]),
            // A symlink is followed to its executable target.
            tree(&[
                ("LICENSE", exec("x")),
                ("debian/copyright", TestFile::symlink("../LICENSE")),
            ]),
            // Config group: a dh command and compat below 9.
            tree(&[
                ("debian/compat", TestFile::new("8\n")),
                ("debian/manpages", exec("x")),
            ]),
            tree(&[
                ("debian/compat", TestFile::new("8\n")),
                ("debian/example.install", exec("x")),
            ]),
        ];
        SourceTreeTester::new(PackageFileIsExecutable, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(tree(&[]))];
        let fail = vec![SourcePackageCase::from(tree(&[(
            "debian/copyright",
            exec("x"),
        )]))];
        SourcePackageTester::new(PackageFileIsExecutable, pass, fail).test_and_snapshot();
    }
}
