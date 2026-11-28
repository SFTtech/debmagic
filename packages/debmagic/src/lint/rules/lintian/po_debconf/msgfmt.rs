//! `msgfmt -o /dev/null --statistics` on each translation in the PO directory,
//! run once per Subject. LN0383 (invalid-po-file) reads the exit status and
//! the po-debconf Check (`pot::state`) reads the statistics on stderr, both
//! from the same run.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use crate::lint::context::SourceContext;
use crate::lint::external_tool::{self, Captured, Sandbox};
use crate::lint::file_tree::{Entry, EntryKind, FileTree};

use super::files;
use super::pot;

/// The `msgfmt` runs of one Subject.
#[derive(Default)]
pub(super) struct Statistics {
    /// `msgfmt` is on the host's `PATH`.
    pub(super) found: bool,
    /// The run on each file for which [`is_checked`] holds. `None` when the
    /// run could not start, timed out, or had no staged tree to run in.
    runs: BTreeMap<PathBuf, Option<Captured>>,
}

impl Statistics {
    /// The completed run on `path`, if there is one.
    pub(super) fn run(&self, path: &Path) -> Option<&Captured> {
        self.runs.get(path)?.as_ref()
    }

    /// Whether `msgfmt` exited non-zero on `path`. False when it did not run.
    pub(super) fn fails(&self, path: &Path) -> bool {
        self.run(path).is_some_and(|run| !run.success())
    }
}

/// A `.po` file in the PO directory with a `msgstr` line in its content, after
/// following symbolic links inside the tree. These are the files `msgfmt`
/// runs on. Directories are not.
pub(super) fn is_checked(tree: &FileTree, entry: &Entry) -> bool {
    if entry.kind() == EntryKind::Directory {
        return false;
    }
    if !files::basename(entry).is_some_and(|name| name.ends_with(".po")) {
        return false;
    }
    let Some(open) = files::open_file(tree, entry.path()) else {
        return false;
    };
    let Ok(bytes) = tree.read(open) else {
        return false;
    };
    bytes
        .split(|&byte| byte == b'\n')
        .any(|line| line.starts_with(b"msgstr"))
}

/// The `msgfmt` runs of `ctx`'s Subject, computed on first use.
pub(super) fn statistics(ctx: &mut SourceContext<'_>) -> Arc<Statistics> {
    ctx.shared_once(|ctx| {
        let Some(tree) = ctx.files() else {
            return Statistics::default();
        };
        let Some(directory) = files::po_directory(tree) else {
            return Statistics::default();
        };
        let paths: Vec<PathBuf> = files::po_children(tree, directory)
            .into_iter()
            .filter(|entry| is_checked(tree, entry))
            .map(files::path_buf)
            .collect();
        let Some(msgfmt) = external_tool::find("msgfmt") else {
            return Statistics {
                found: false,
                runs: BTreeMap::new(),
            };
        };
        let outcomes = if paths.is_empty() {
            Vec::new()
        } else {
            let staged = pot::staged(ctx);
            match staged.as_ref() {
                Some(sandbox) => run_all(sandbox, &msgfmt, &paths),
                None => vec![None; paths.len()],
            }
        };
        Statistics {
            found: true,
            runs: paths.into_iter().zip(outcomes).collect(),
        }
    })
}

/// `msgfmt -o /dev/null --statistics` on `path` (a tree path) in `sandbox`.
/// The arguments and environment are the ones both users always used.
fn run_one(sandbox: &Sandbox, msgfmt: &Path, path: &Path) -> Option<Captured> {
    sandbox
        .run(
            msgfmt,
            &[
                OsStr::new("-o"),
                OsStr::new("/dev/null"),
                OsStr::new("--statistics"),
                path.as_os_str(),
            ],
            Path::new(""),
            &[],
            external_tool::DEFAULT_TIMEOUT,
        )
        .ok()
}

/// [`run_one`] for each path, in parallel, with results in input order.
///
/// Plain scoped threads, not rayon: this runs inside a `shared_once`
/// initializer that other Rules wait on. A rayon worker waiting here could
/// steal a Rule that waits for this initializer, which deadlocks.
fn run_all(sandbox: &Sandbox, msgfmt: &Path, paths: &[PathBuf]) -> Vec<Option<Captured>> {
    let threads = rayon::current_num_threads().min(paths.len());
    if threads == 0 {
        return Vec::new();
    }
    let next = AtomicUsize::new(0);
    let done: Vec<Vec<(usize, Option<Captured>)>> = thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(path) = paths.get(index) else {
                            return done;
                        };
                        done.push((index, run_one(sandbox, msgfmt, path)));
                    }
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|worker| {
                worker
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    });
    let mut all: Vec<(usize, Option<Captured>)> = done.into_iter().flatten().collect();
    all.sort_by_key(|(index, _)| *index);
    all.into_iter().map(|(_, run)| run).collect()
}
