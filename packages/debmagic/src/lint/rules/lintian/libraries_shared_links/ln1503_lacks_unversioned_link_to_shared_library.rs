use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::elf::ElfFiles;
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::group_context::GroupContext;
use crate::lint::rule::GroupRule;

use super::ln1413_link_to_shared_library_in_wrong_package::unversioned_name;
use super::shared::shared_libraries;

pub struct LacksUnversionedLinkToSharedLibrary;

declare_rule! {
    /// A shared library in an ldconfig folder has a `-dev` package, but no
    /// package of the Group has an unversioned `.so` symlink to it. The
    /// Diagnostic is on the library's package, at the library, with extra
    /// `example: <path of the link that would do>`.
    ///
    /// A `-dev` package of the Group counts when its name ends in `-dev` and
    /// its Pre-Depends or Depends satisfy the library's package. Its symlinks
    /// under `usr/lib/` are followed (an absolute target, or one relative to
    /// the root of the library's package first, else relative to the
    /// symlink); the library is linked when one that reaches it is named
    /// `*.so`. The example path is the library's SONAME path without its
    /// version, under `usr/` when the library is not (a library in `/lib`
    /// needs its link in `/usr/lib`).
    ///
    /// A Group Rule: it needs the `-dev` packages' files, so it runs once per
    /// Group that has a `.dsc`, and the Diagnostic attaches to the library's
    /// installable (binary or udeb), as in lintian.
    LacksUnversionedLinkToSharedLibrary,
    code = "LN1503",
    tag = "lacks-unversioned-link-to-shared-library",
    default_selected = true,
    experimental = false,
}

impl GroupRule for LacksUnversionedLinkToSharedLibrary {
    fn run(&self, ctx: &mut GroupContext<'_>) {
        let installables = ctx.installables();
        for installable in &installables {
            let development: Vec<_> = installables
                .iter()
                .filter(|other| {
                    other.name().ends_with("-dev")
                        && other.strong_relation().satisfies(installable.name())
                })
                .collect();
            if development.is_empty() {
                continue;
            }
            let Some(files) = ctx.data_files(installable) else {
                continue;
            };
            let elf = ElfFiles::from_tree(files);
            let mut development_trees = Vec::new();
            for other in &development {
                if let Some(tree) = ctx.data_files(other) {
                    development_trees.push(tree);
                }
            }

            let mut findings: Vec<(PathBuf, String)> = Vec::new();
            for (library, versioned) in shared_libraries(files, &elf) {
                let mut unversioned = unversioned_name(&versioned);
                // A library in /lib has its link in /usr/lib.
                if !library.starts_with("usr") {
                    unversioned = format!("usr/{unversioned}");
                }
                let linked = development_trees.iter().any(|tree| {
                    tree.entries()
                        .iter()
                        .filter(|entry| entry.kind() == EntryKind::Symlink)
                        .filter(|entry| entry.path().starts_with("usr/lib/"))
                        .filter(|link| reaches(files, link, &library))
                        .any(|link| link.path().to_string_lossy().ends_with(".so"))
                });
                if !linked {
                    findings.push((library, format!("example: {unversioned}")));
                }
            }
            for (path, extra) in findings {
                ctx.installable_diagnostic(
                    installable,
                    Diagnostic::warn(extra).with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
                );
            }
        }
    }
}

/// The symlink `link` of a `-dev` package leads to `library` in the library
/// package's `files`: tried from the root first, then from the symlink's
/// directory.
fn reaches(files: &FileTree, link: &Entry, library: &Path) -> bool {
    let Some(target) = link.link_target() else {
        return false;
    };
    let resolved = resolve(files, Path::new(""), target).or_else(|| {
        let directory = link.path().parent().unwrap_or(Path::new(""));
        resolve(files, directory, target)
    });
    resolved.is_some_and(|entry| entry.path() == library)
}

/// lintian's `resolve_path` on a File tree: `target` from `directory` (from
/// the root when absolute), following symlinks on the way and at the end. A
/// directory with no Entry of its own is taken to exist, as long as the
/// final Entry does.
fn resolve<'a>(files: &'a FileTree, directory: &Path, target: &Path) -> Option<&'a Entry> {
    let mut current: Vec<OsString> = directory
        .components()
        .map(|component| component.as_os_str().to_os_string())
        .collect();
    let mut pending: VecDeque<OsString> = VecDeque::new();
    push_front(&mut pending, &mut current, target);
    let mut hops = 0;
    while let Some(name) = pending.pop_front() {
        if name == "." {
            continue;
        }
        if name == ".." {
            current.pop()?;
            continue;
        }
        current.push(name);
        let path: PathBuf = current.iter().collect();
        let is_last = pending.is_empty();
        match files.get(&path) {
            Some(entry) if entry.kind() == EntryKind::Symlink => {
                hops += 1;
                if hops > 40 {
                    return None;
                }
                let target = entry.link_target()?.to_path_buf();
                current.pop();
                push_front(&mut pending, &mut current, &target);
                if pending.is_empty() {
                    return None;
                }
            }
            Some(_) => {}
            None if is_last => return None,
            None => {}
        }
    }
    if current.is_empty() {
        return None;
    }
    files.get(current.iter().collect::<PathBuf>())
}

/// Queue the components of `target` ahead of the pending ones; an absolute
/// target starts from the root.
fn push_front(pending: &mut VecDeque<OsString>, current: &mut Vec<OsString>, target: &Path) {
    if target.is_absolute() {
        current.clear();
    }
    let names: Vec<OsString> = target
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(name) => Some(name.to_os_string()),
            std::path::Component::CurDir => Some(OsString::from(".")),
            std::path::Component::ParentDir => Some(OsString::from("..")),
            _ => None,
        })
        .collect();
    for name in names.into_iter().rev() {
        pending.push_front(name);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{GroupCase, GroupTester, TestFile};

    use super::*;

    const DSC: &str = "Source: example\nVersion: 1.0\n";

    fn elf(soname: &str) -> TestFile {
        TestFile::from_bytes(TestElf::shared_object().soname(soname).build())
    }

    /// A library package, and a `-dev` package that depends on it.
    fn group(
        library: Vec<(&'static str, TestFile)>,
        dev: Vec<(&'static str, TestFile)>,
    ) -> GroupCase {
        GroupCase::new(DSC)
            .installable("libfoo1", "")
            .files(library)
            .installable("libfoo-dev", "Depends: libfoo1 (= 1.0)\n")
            .files(dev)
    }

    fn lib() -> Vec<(&'static str, TestFile)> {
        vec![("usr/lib/libfoo.so.1.0.1", elf("libfoo.so.1"))]
    }

    #[test]
    fn group_rule() {
        let pass = vec![
            // The link, relative.
            group(
                lib(),
                vec![("usr/lib/libfoo.so", TestFile::symlink("libfoo.so.1.0.1"))],
            ),
            // Through the SONAME link.
            group(
                vec![
                    ("usr/lib/libfoo.so.1.0.1", elf("libfoo.so.1")),
                    ("usr/lib/libfoo.so.1", TestFile::symlink("libfoo.so.1.0.1")),
                ],
                vec![("usr/lib/libfoo.so", TestFile::symlink("libfoo.so.1"))],
            ),
            // An absolute target.
            group(
                lib(),
                vec![(
                    "usr/lib/libfoo.so",
                    TestFile::symlink("/usr/lib/libfoo.so.1.0.1"),
                )],
            ),
            // A relative target with `..`, link in a deeper directory.
            group(
                lib(),
                vec![(
                    "usr/lib/foo/libfoo.so",
                    TestFile::symlink("../libfoo.so.1.0.1"),
                )],
            ),
            // A library in /lib, link in /usr/lib.
            group(
                vec![("lib/libfoo.so.1.0.1", elf("libfoo.so.1"))],
                vec![(
                    "usr/lib/libfoo.so",
                    TestFile::symlink("/lib/libfoo.so.1.0.1"),
                )],
            ),
            // No -dev package depends on the library package.
            GroupCase::new(DSC)
                .installable("libfoo1", "")
                .files(lib())
                .installable("libfoo-dev", "Depends: other\n"),
            GroupCase::new(DSC).installable("libfoo1", "").files(lib()),
            // A weak relation does not make a -dev package of it.
            GroupCase::new(DSC)
                .installable("libfoo1", "")
                .files(lib())
                .installable("libfoo-dev", "Recommends: libfoo1\n"),
            // Not a shared library in a library folder, or without a SONAME.
            group(vec![("opt/libfoo.so.1.0.1", elf("libfoo.so.1"))], vec![]),
            group(
                vec![(
                    "usr/lib/libfoo.so.1.0.1",
                    TestFile::from_bytes(TestElf::shared_object().build()),
                )],
                vec![],
            ),
        ];
        let fail = vec![
            group(lib(), vec![]),
            // A link that does not end in .so, or goes elsewhere.
            group(
                lib(),
                vec![("usr/lib/libfoo.so.1", TestFile::symlink("libfoo.so.1.0.1"))],
            ),
            group(
                lib(),
                vec![("usr/lib/libfoo.so", TestFile::symlink("libbar.so.1"))],
            ),
            // A link outside usr/lib, and a regular file named .so.
            group(
                lib(),
                vec![(
                    "lib/libfoo.so",
                    TestFile::symlink("/usr/lib/libfoo.so.1.0.1"),
                )],
            ),
            group(lib(), vec![("usr/lib/libfoo.so", TestFile::new("x"))]),
            // A library in /lib asks for the link in /usr/lib; a udeb is looked at too.
            group(vec![("lib/libfoo.so.1.0.1", elf("libfoo.so.1"))], vec![]),
            // A library with no version in its name.
            group(vec![("usr/lib/libbaz.so", elf("libbaz.so"))], vec![]),
            GroupCase::new(DSC)
                .udeb("libfoo1-udeb", "")
                .files(lib())
                .installable("libfoo-dev", "Depends: libfoo1-udeb\n"),
        ];
        GroupTester::new(LacksUnversionedLinkToSharedLibrary, pass, fail).test_and_snapshot();
    }
}
