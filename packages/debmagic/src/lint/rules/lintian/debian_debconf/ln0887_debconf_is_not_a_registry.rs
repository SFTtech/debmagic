use std::path::{Component, Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::interpreter::Hashbang;
use crate::lint::rule::BinaryPackageRule;

use super::files;

pub struct DebconfIsNotARegistry;

declare_rule! {
    /// An installed script loads the debconf confmodule.
    ///
    /// Scripts are the Data files that start with `#!` (`Hashbang`), and
    /// symlinks count when they resolve inside the package. The first line
    /// with `/usr/share/debconf/confmodule`, `Debconf::Client::ConfModule`, or
    /// `Debian::DebConf::Client::ConfModule` is reported once per script, at
    /// that line, with an empty Extra. Comments are dropped first. Packages
    /// named `debconf` and udebs are exempt.
    DebconfIsNotARegistry,
    code = "LN0887",
    tag = "debconf-is-not-a-registry",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DebconfIsNotARegistry {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() || ctx.package_name() == "debconf" {
            return;
        }
        let hits = {
            let Some(tree) = ctx.data_files() else {
                return;
            };
            let mut entries: Vec<&Entry> = tree.entries().iter().collect();
            entries.sort_by(|left, right| left.path().as_os_str().cmp(right.path().as_os_str()));
            let mut hits: Vec<(PathBuf, u32)> = Vec::new();
            for entry in entries {
                let Some(target) = open_target(tree, entry) else {
                    continue;
                };
                let Ok(bytes) = tree.read(target) else {
                    continue;
                };
                if Hashbang::read(&bytes).is_none() {
                    continue;
                }
                if let Some(line) = registry_line(&bytes) {
                    hits.push((entry.path().to_path_buf(), line));
                }
            }
            hits
        };
        for (path, line) in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(files::located(path, Some(line))));
        }
    }
}

/// The 1-based line of the first confmodule mention, comments dropped.
fn registry_line(bytes: &[u8]) -> Option<u32> {
    let text = String::from_utf8_lossy(bytes);
    text.split('\n').enumerate().find_map(|(index, line)| {
        let code = line.split('#').next().unwrap_or("");
        let loads = code.contains("/usr/share/debconf/confmodule")
            || code.contains("Debconf::Client::ConfModule")
            || code.contains("Debian::DebConf::Client::ConfModule");
        loads.then(|| u32::try_from(index + 1).unwrap_or(u32::MAX))
    })
}

/// The regular file or hardlink an entry opens as, following symlinks that
/// stay inside the tree. `None` when it does not open.
fn open_target<'t>(tree: &'t FileTree, entry: &'t Entry) -> Option<&'t Entry> {
    let mut current = entry;
    for _ in 0..8 {
        match current.kind() {
            EntryKind::RegularFile | EntryKind::Hardlink => return Some(current),
            EntryKind::Directory
            | EntryKind::Fifo
            | EntryKind::CharDevice
            | EntryKind::BlockDevice => return None,
            EntryKind::Symlink => {
                let target = current.link_target()?;
                let next = resolve_link(current.path(), target)?;
                current = tree.get(&next)?;
            }
        }
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
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn data(path: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(body))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            data("usr/share/doc/x/notes", ". /usr/share/debconf/confmodule\n"),
            data(
                "usr/share/lintian/script",
                "#!/bin/sh\n# . /usr/share/debconf/confmodule\n",
            ),
            data(
                "usr/share/lintian/lincity",
                "#!#!#!/bin/sh\n. /usr/share/debconf/confmodule\n",
            ),
            data(
                "usr/share/lintian/script",
                "#!/bin/sh\n. /usr/share/debconf/confmodule\n",
            )
            .udeb(),
            data(
                "usr/share/lintian/script",
                "#!/bin/sh\n. /usr/share/debconf/confmodule\n",
            )
            .package("debconf"),
        ];
        let fail = vec![
            data(
                "usr/share/lintian/script",
                "#!/bin/sh\n\n. /usr/share/debconf/confmodule\n",
            ),
            data(
                "usr/bin/tool",
                "#!/usr/bin/perl\nuse Debconf::Client::ConfModule;\n",
            ),
            data(
                "usr/bin/tool",
                "#!/usr/bin/perl\nuse Debian::DebConf::Client::ConfModule;\n",
            ),
            BinaryPackageCase::from(vec![
                (
                    "usr/bin/real",
                    TestFile::new("#!/bin/sh\n. /usr/share/debconf/confmodule\n"),
                ),
                ("usr/bin/link", TestFile::symlink("real")),
            ]),
        ];
        BinaryPackageTester::new(DebconfIsNotARegistry, pass, fail).test_and_snapshot();
    }
}
