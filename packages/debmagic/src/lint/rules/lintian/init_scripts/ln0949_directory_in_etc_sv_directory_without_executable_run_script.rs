use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

pub struct DirectoryInEtcSvDirectoryWithoutExecutableRunScript;

declare_rule! {
    /// A directory `etc/sv/<service>/` has no executable `run` script.
    ///
    /// Every directory whose parent is `etc/sv` is a service. Its `run` file
    /// is followed through symlinks and must be a regular file or hardlink
    /// with an execute bit. Otherwise the Rule reports the service directory.
    /// The extra is the path the `run` link resolves to, or empty when there
    /// is no `run` at all. Subdirectories such as `etc/sv/<service>/log` are
    /// not services. The directory is the Location, without lintian's trailing
    /// slash.
    DirectoryInEtcSvDirectoryWithoutExecutableRunScript,
    code = "LN0949",
    tag = "directory-in-etc-sv-directory-without-executable-run-script",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DirectoryInEtcSvDirectoryWithoutExecutableRunScript {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(data) = ctx.data_files() else {
            return;
        };
        let findings: Vec<(PathBuf, String)> = service_dirs(data)
            .into_iter()
            .filter_map(|dir| match resolve(data, &dir.join("run")) {
                Some(run) if run.is_executable_file() => None,
                Some(run) => Some((dir, run.path().to_string_lossy().into_owned())),
                None => Some((dir, String::new())),
            })
            .collect();
        for (dir, extra) in findings {
            ctx.diagnostic(Diagnostic::error(extra).with_location(at(&dir)));
        }
    }
}

/// Directories whose parent path ends in `etc/sv`, as lintian's
/// `etc/sv/([^/]+)/$` match sees them.
fn service_dirs(data: &FileTree) -> Vec<PathBuf> {
    data.entries()
        .iter()
        .filter(|entry| entry.kind() == EntryKind::Directory)
        .filter(|entry| {
            entry
                .path()
                .parent()
                .is_some_and(|parent| parent.as_os_str().as_bytes().ends_with(b"etc/sv"))
        })
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

/// Follows symlinks to the entry they end at. Absolute targets are relative
/// to the package root. A broken link or a loop resolves to nothing.
fn resolve<'a>(data: &'a FileTree, path: &Path) -> Option<&'a Entry> {
    let mut entry = data.get(path)?;
    for _ in 0..8 {
        if entry.kind() != EntryKind::Symlink {
            return Some(entry);
        }
        let target = entry.link_target()?;
        let mut resolved = if target.is_absolute() {
            PathBuf::new()
        } else {
            entry
                .path()
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default()
        };
        for component in target.components() {
            match component {
                Component::Normal(part) => resolved.push(part),
                Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
                Component::ParentDir => {
                    if !resolved.pop() {
                        return None;
                    }
                }
            }
        }
        entry = data.get(&resolved)?;
    }
    None
}

fn at(path: &Path) -> Location {
    Location {
        path: path.to_path_buf(),
        line: None,
        column: None,
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(
            files
                .into_iter()
                .map(|(path, file)| (path.to_string(), file))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn binary() {
        let script = || TestFile::new("#!/bin/sh\n");
        let exec = || script().mode(0o755);
        let pass = vec![
            case(vec![
                ("etc/sv/foo", TestFile::directory()),
                ("etc/sv/foo/run", exec()),
            ]),
            case(vec![
                ("etc/sv/foo", TestFile::directory()),
                ("etc/sv/foo/exec", exec()),
                ("etc/sv/foo/run", TestFile::symlink("exec")),
            ]),
            case(vec![
                ("etc/sv/foo", TestFile::directory()),
                ("etc/sv/foo/log", TestFile::directory()),
                ("etc/sv/foo/run", exec()),
            ]),
            case(vec![
                ("etc/sv", TestFile::directory()),
                ("etc/default/foo", script()),
            ]),
        ];
        let fail = vec![
            case(vec![("etc/sv/foo", TestFile::directory())]),
            case(vec![
                ("etc/sv/foo", TestFile::directory()),
                ("etc/sv/foo/run", script()),
            ]),
            case(vec![
                ("etc/sv/foo", TestFile::directory()),
                ("etc/sv/foo/exec", script()),
                ("etc/sv/foo/run", TestFile::symlink("exec")),
            ]),
            case(vec![
                ("etc/sv/foo", TestFile::directory()),
                ("etc/sv/foo/run", TestFile::symlink("gone")),
            ]),
            case(vec![("etc/sv/foo", TestFile::directory())]).udeb(),
        ];
        BinaryPackageTester::new(
            DirectoryInEtcSvDirectoryWithoutExecutableRunScript,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
