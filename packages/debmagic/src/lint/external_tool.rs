//! Host programs that Rules run on a subset of a File tree.
//!
//! lintian's Checks call host tools (`safe_qx`, `system`) on the unpacked
//! files. This module does the same for debmagic: the Entries a Rule names
//! are written into a private temporary directory (exact bytes, permission
//! bits, symlinks as symlinks, directories), the program runs there with that
//! directory as its working directory, and its status, stdout and stderr come
//! back. The directory is removed when the run ends.
//!
//! The environment is cleared and replaced with `LC_ALL=C` and a fixed `PATH`,
//! stdin is empty, and the run is killed after its timeout. The network is not
//! isolated: a tool that reaches the network can do so, as it can under
//! lintian.

use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use tempfile::TempDir;

use super::file_tree::{Entry, FileTree};

/// How long a tool may run before it is killed.
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// The `PATH` a tool runs with, whatever the host's own.
const TOOL_PATH: &str = "/usr/bin:/bin";

/// What a finished run reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Captured {
    /// The exit code; `None` when the program was killed by a signal.
    pub(crate) code: Option<i32>,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

impl Captured {
    /// Exit status zero.
    pub(crate) fn success(&self) -> bool {
        self.code == Some(0)
    }
}

/// `which`: the path of an executable named `program`, or `None` when the host
/// has none. A program with a `/` in it is checked as given. Otherwise the
/// first executable of that name on `PATH` is returned.
pub(crate) fn find(program: &str) -> Option<PathBuf> {
    if program.contains('/') {
        let path = PathBuf::from(program);
        return is_executable(&path).then_some(path);
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(program))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// Writes `entries` of `files` below `root`, at their tree paths.
///
/// Regular files and hardlinks get their bytes and permission bits; symlinks
/// are recreated as symlinks with their target; directories are created.
/// Devices and FIFOs are not materialised.
pub(crate) fn materialise(files: &FileTree, entries: &[&Entry], root: &Path) -> Result<()> {
    for entry in entries {
        let dest = root.join(entry.path());
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        match entry {
            Entry::RegularFile { .. } | Entry::Hardlink { .. } => {
                fs::write(&dest, files.read(entry)?)
                    .with_context(|| format!("writing {}", dest.display()))?;
                fs::set_permissions(&dest, fs::Permissions::from_mode(entry.mode()))
                    .with_context(|| format!("setting mode of {}", dest.display()))?;
            }
            Entry::Symlink { .. } => {
                let target = entry.link_target().unwrap_or(Path::new(""));
                symlink(target, &dest).with_context(|| format!("linking {}", dest.display()))?;
            }
            Entry::Directory { .. } => {
                fs::create_dir_all(&dest)
                    .with_context(|| format!("creating {}", dest.display()))?;
            }
            Entry::Fifo { .. } | Entry::CharDevice { .. } | Entry::BlockDevice { .. } => {}
        }
    }
    Ok(())
}

/// A private directory holding a File-tree subset, for running host tools in.
///
/// Built once with the Entries a run needs, then used for any number of
/// [`Sandbox::run`] calls, so a sequence of tools can share the files they
/// write. The directory is removed when the Sandbox is dropped.
pub(crate) struct Sandbox {
    dir: TempDir,
}

impl Sandbox {
    /// A Sandbox with `entries` of `files` written at their tree paths.
    pub(crate) fn new(files: &FileTree, entries: &[&Entry]) -> Result<Self> {
        let dir = tempfile::Builder::new()
            .prefix("debmagic-tool-")
            .tempdir()
            .context("creating a scratch directory")?;
        materialise(files, entries, dir.path())?;
        Ok(Self { dir })
    }

    /// The directory on the host. Tree paths are relative to it.
    pub(crate) fn root(&self) -> &Path {
        self.dir.path()
    }

    /// Runs `program` with `args` from `cwd` (a tree path, `""` for the root).
    /// `env` is added to the cleared environment.
    ///
    /// Errors when the program cannot start or runs longer than `timeout`
    /// (then it is killed). A non-zero exit is not an error; read
    /// [`Captured::success`].
    pub(crate) fn run(
        &self,
        program: &Path,
        args: &[&OsStr],
        cwd: &Path,
        env: &[(&str, &OsStr)],
        timeout: Duration,
    ) -> Result<Captured> {
        let mut stdout = tempfile::tempfile().context("creating stdout capture")?;
        let mut stderr = tempfile::tempfile().context("creating stderr capture")?;
        let mut child = Command::new(program)
            .args(args)
            .current_dir(self.root().join(cwd))
            .env_clear()
            .env("LC_ALL", "C")
            .env("PATH", TOOL_PATH)
            .envs(env.iter().copied())
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout.try_clone()?))
            .stderr(Stdio::from(stderr.try_clone()?))
            .spawn()
            .with_context(|| format!("starting {}", program.display()))?;

        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if started.elapsed() >= timeout {
                let _ = child.kill();
                let _ = child.wait();
                bail!("{} ran longer than {timeout:?}", program.display());
            }
            thread::sleep(Duration::from_millis(5));
        };

        Ok(Captured {
            code: status.code(),
            stdout: read_back(&mut stdout)?,
            stderr: read_back(&mut stderr)?,
        })
    }
}

/// lintian's `safe_qx(program, args, unpacked_path)` followed by `$?`: whether
/// `program` exits non-zero on `entry` alone, with `args` before its path.
/// `None` when the entry cannot be written or the run fails, so there is no
/// verdict.
pub(crate) fn rejects(
    files: &FileTree,
    entry: &Entry,
    program: &Path,
    args: &[&str],
) -> Option<bool> {
    let sandbox = Sandbox::new(files, &[entry]).ok()?;
    let mut argv: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
    argv.push(entry.path().as_os_str());
    sandbox
        .run(program, &argv, Path::new(""), &[], DEFAULT_TIMEOUT)
        .ok()
        .map(|captured| !captured.success())
}

fn read_back(capture: &mut File) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    capture
        .seek(SeekFrom::Start(0))
        .and_then(|_| capture.read_to_end(&mut bytes))
        .map_err(|error| anyhow!("reading captured output: {error}"))?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(entries: Vec<Entry>) -> FileTree {
        FileTree::from_entries(None, entries)
    }

    fn file(path: &str, content: &[u8], mode: u32) -> Entry {
        Entry::regular_file(path, None, Some(content.to_vec())).with_mode(mode)
    }

    #[test]
    fn find_absolute_and_path_lookup() {
        assert_eq!(find("/bin/sh"), Some(PathBuf::from("/bin/sh")));
        assert_eq!(find("/no/such/tool"), None);
        assert_eq!(find("no-such-debmagic-tool-xyz"), None);
        // A directory is not an executable file.
        assert_eq!(find("/bin"), None);
    }

    #[test]
    fn materialise_writes_bytes_modes_and_links() {
        let files = tree(vec![
            file("usr/bin/tool", b"#!/bin/sh\n", 0o750),
            Entry::symlink("usr/bin/alias", "tool"),
            Entry::directory("usr/share/empty"),
        ]);
        let entries: Vec<&Entry> = files.entries().iter().collect();
        let root = tempfile::tempdir().expect("tempdir");
        materialise(&files, &entries, root.path()).expect("materialise");

        let tool = root.path().join("usr/bin/tool");
        assert_eq!(fs::read(&tool).expect("read"), b"#!/bin/sh\n");
        assert_eq!(
            fs::metadata(&tool).expect("meta").permissions().mode() & 0o777,
            0o750
        );
        assert_eq!(
            fs::read_link(root.path().join("usr/bin/alias")).expect("link"),
            Path::new("tool")
        );
        assert!(root.path().join("usr/share/empty").is_dir());
    }

    #[test]
    fn run_sees_only_the_materialised_entries() {
        let files = tree(vec![file("a/b", b"hello\n", 0o644)]);
        let entries: Vec<&Entry> = files.entries().iter().collect();
        let sandbox = Sandbox::new(&files, &entries).expect("sandbox");
        let run = |arg: &str| {
            sandbox
                .run(
                    Path::new("/bin/cat"),
                    &[OsStr::new(arg)],
                    Path::new(""),
                    &[],
                    DEFAULT_TIMEOUT,
                )
                .expect("run")
        };

        let found = run("a/b");
        assert!(found.success());
        assert_eq!(found.stdout, b"hello\n");
        assert!(found.stderr.is_empty());

        let missing = run("a/missing");
        assert!(!missing.success());
        assert!(!missing.stderr.is_empty());
    }

    #[test]
    fn run_reports_exit_status_and_env() {
        let files = tree(vec![]);
        let sandbox = Sandbox::new(&files, &[]).expect("sandbox");
        let status = sandbox
            .run(
                Path::new("/bin/false"),
                &[],
                Path::new(""),
                &[],
                DEFAULT_TIMEOUT,
            )
            .expect("run");
        assert_eq!(status.code, Some(1));
        assert!(!status.success());

        let env = sandbox
            .run(
                Path::new("/bin/sh"),
                &[OsStr::new("-c"), OsStr::new("printf %s \"$GREETING\"")],
                Path::new(""),
                &[("GREETING", OsStr::new("hi"))],
                DEFAULT_TIMEOUT,
            )
            .expect("run");
        assert_eq!(env.stdout, b"hi");
    }

    #[test]
    fn run_is_killed_after_the_timeout() {
        let files = tree(vec![]);
        let sandbox = Sandbox::new(&files, &[]).expect("sandbox");
        let result = sandbox.run(
            Path::new("/bin/sleep"),
            &[OsStr::new("30")],
            Path::new(""),
            &[],
            Duration::from_millis(200),
        );
        assert!(result.is_err());
    }
}
