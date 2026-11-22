use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, follow_symlinks};
use crate::lint::data;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::Entry;
use crate::lint::rule::BinaryPackageRule;
use crate::lint::rules::lintian::fields_relations_more::architectures::is_ldconfig_folder;

pub struct LacksLdconfigTrigger;

declare_rule! {
    /// A Binary package installs a shared library into an `ldconfig` folder
    /// without activating libc's `ldconfig` trigger.
    ///
    /// Reports the Data files that `ldconfig` must see: each regular file,
    /// hardlink or symlink whose path, or whose link target normalized against
    /// its directory, is a file with an ELF `SONAME`, and whose own directory is
    /// an `ldconfig` folder. A directory whose name is a hardware capability
    /// (`data/shared-libs/hwcap-dirs`, such as `tls`) is looked through, as the
    /// Check does. The names are sorted, de-duplicated and joined with a space
    /// into one Diagnostic with no Location.
    ///
    /// Skipped for udebs. The trigger counts as activated when the `triggers`
    /// Control file (a symlink followed) has a line `activate-noawait ldconfig`.
    /// Link targets are not chased through further symlinks, as lintian does not.
    LacksLdconfigTrigger,
    code = "LN1414",
    tag = "lacks-ldconfig-trigger",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for LacksLdconfigTrigger {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let must_call = must_call_ldconfig(ctx);
        if must_call.is_empty() || triggers_ldconfig(ctx) {
            return;
        }
        let names: Vec<String> = must_call
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        ctx.diagnostic(Diagnostic::error(names.join(" ")));
    }
}

/// Whether the `triggers` Control file has a line `activate-noawait ldconfig`
/// (Perl `/^ \s* activate-noawait \s+ ldconfig \s* $/mx`). Invalid UTF-8 counts
/// as no trigger.
pub(crate) fn triggers_ldconfig(ctx: &mut BinaryPackageContext<'_>) -> bool {
    let Some(tree) = ctx.control_files() else {
        return false;
    };
    let Some(entry) = tree.get("triggers") else {
        return false;
    };
    let Some(target) = follow_symlinks(tree, entry) else {
        return false;
    };
    let Ok(bytes) = tree.read(target) else {
        return false;
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return false;
    };
    text.lines().any(|line| {
        let mut words = line.split_whitespace();
        words.next() == Some("activate-noawait")
            && words.next() == Some("ldconfig")
            && words.next().is_none()
    })
}

/// The Installed files lintian's `ldconfig` Check collects in
/// `must_call_ldconfig`: sorted and unique. Each is the resolved name of an
/// installed file with an ELF `SONAME` that lies in an `ldconfig` folder.
pub(crate) fn must_call_ldconfig(ctx: &mut BinaryPackageContext<'_>) -> BTreeSet<PathBuf> {
    let mut found = BTreeSet::new();
    let Some(elf_files) = ctx.data_elf_files() else {
        return found;
    };
    let sonames: BTreeSet<&Path> = elf_files
        .iter()
        .filter(|(_, facts)| !facts.soname().is_empty())
        .map(|(path, _)| path)
        .collect();
    let Some(files) = ctx.data_files() else {
        return found;
    };
    for entry in files.entries() {
        let resolved = match entry_link(entry) {
            Some((from_root, target)) => match normalize_link(entry, from_root, target) {
                Some(resolved) => resolved,
                None => continue,
            },
            None => entry.path().to_path_buf(),
        };
        if !sonames.contains(resolved.as_path()) {
            continue;
        }
        if needs_ldconfig(entry.path()) {
            found.insert(resolved);
        }
    }
    found
}

/// The link target of a symlink or hardlink, and whether a hardlink (which is
/// relative to the package root, not to its directory).
fn entry_link(entry: &Entry) -> Option<(bool, &Path)> {
    if let Some(target) = entry.link_target() {
        return Some((false, target));
    }
    entry.hardlink_target().map(|target| (true, target))
}

/// Lintian's `link_normalized`: the target against the link's directory (the
/// package root for a hardlink), `.` and `..` folded. `None` when it climbs
/// above the root.
fn normalize_link(entry: &Entry, from_root: bool, target: &Path) -> Option<PathBuf> {
    let joined = if target.has_root() {
        target.to_path_buf()
    } else if from_root {
        Path::new("/").join(target)
    } else {
        entry.path().parent().unwrap_or(Path::new("")).join(target)
    };
    let mut parts: Vec<&std::ffi::OsStr> = Vec::new();
    for component in joined.components() {
        match component {
            Component::Normal(name) => parts.push(name),
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
        }
    }
    Some(parts.iter().collect())
}

/// Lintian's `needs_ldconfig` on the directory of an installed file: strip
/// trailing hardware-capability directories, then ask whether the rest is an
/// `ldconfig` folder.
fn needs_ldconfig(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    let mut parts: Vec<&str> = parent
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => name.to_str(),
            _ => None,
        })
        .collect();
    loop {
        let Some(last) = parts.pop() else {
            return false;
        };
        if !hwcap_dir(last) {
            parts.push(last);
            break;
        }
    }
    let dir = parts.join("/");
    !dir.is_empty() && is_ldconfig_folder(&dir)
}

/// Whether `name` is in `data/shared-libs/hwcap-dirs`.
fn hwcap_dir(name: &str) -> bool {
    data::lines(include_str!("../../../data/shared-libs/hwcap-dirs")).any(|line| line == name)
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn library() -> TestFile {
        TestFile::from_bytes(TestElf::shared_object().soname("libfoo.so.1").build())
    }

    #[test]
    fn binary() {
        let pass = vec![
            // Triggered.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1", library())])
                .control_file("triggers", TestFile::new("activate-noawait ldconfig\n")),
            // Not in an ldconfig folder.
            BinaryPackageCase::from(vec![("usr/lib/foo/libfoo.so.1", library())]),
            // Hardware capability directory is looked through, and the folder
            // beyond it is not an ldconfig folder.
            BinaryPackageCase::from(vec![("usr/share/foo/tls/libfoo.so.1", library())]),
            // A shared object without SONAME is not a library to register.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so",
                TestFile::from_bytes(TestElf::shared_object().build()),
            )]),
            // A udeb is not checked.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1", library())]).udeb(),
        ];
        let fail = vec![
            // No trigger at all.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1", library())]),
            // Hardware capability directory is looked through.
            BinaryPackageCase::from(vec![("usr/lib/tls/libfoo.so.1", library())]),
            // Symlink to a library with SONAME in an ldconfig folder reports the
            // link target.
            BinaryPackageCase::from(vec![
                ("usr/lib/libfoo.so.1.0.1", library()),
                ("usr/lib/libfoo.so", TestFile::symlink("libfoo.so.1.0.1")),
            ]),
            // Hardlink reports the linked member.
            BinaryPackageCase::from(vec![
                ("usr/lib/libfoo.so.1.0.1", library()),
                (
                    "usr/lib/libfoo.so.1",
                    TestFile::new("").hardlink_to("usr/lib/libfoo.so.1.0.1"),
                ),
            ]),
            // A trigger line that is not the whole directive does not count.
            BinaryPackageCase::from(vec![("usr/lib/libfoo.so.1", library())])
                .control_file("triggers", TestFile::new("activate ldconfig\n")),
            // Multiarch triplet folder.
            BinaryPackageCase::from(vec![("usr/lib/x86_64-linux-gnu/libfoo.so.1", library())]),
        ];
        BinaryPackageTester::new(LacksLdconfigTrigger, pass, fail).test_and_snapshot();
    }
}
