//! Staging of the source tree into the build directory.
//!
//! Which files are staged is selected by `SourceSyncMode`: git-tracked
//! files (optionally requiring a clean worktree) or everything that isn't
//! git-ignored. Staging copies the selected entries, preserving symlinks
//! and keeping unchanged files untouched so incremental builds keep their
//! outputs; a manifest of staged entries lets incremental syncs remove
//! stale paths.

use std::cmp::Reverse;
use std::ffi::OsStr;
use std::io::{self, BufReader, Read};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Component, Path, PathBuf};
use std::{fs, process::Command};

use anyhow::{Context, anyhow, bail};
use glob::glob;

use clap::ValueEnum;

use crate::build_intent::BuildKind;
use crate::driver::Environment;
use crate::upstream::orig::link_or_copy;
use debmagic_common::package::SourcePackage;

/// The staged tree a build kind uses inside the work dir. The `-binary` tree
/// holds incremental build outputs and must never be wiped by a source
/// build; the `-source` tree is restaged instead. Both share the
/// environment (container, apt cache).
pub fn stage_dir(environment: &Environment, kind: BuildKind) -> PathBuf {
    let suffix = match kind {
        BuildKind::Binary => "binary",
        BuildKind::Source => "source",
    };
    environment
        .work_dir()
        .join(format!("{}-{suffix}", environment.package_identifier))
}

pub fn source_manifest_path(environment: &Environment, kind: BuildKind) -> PathBuf {
    let kind = match kind {
        BuildKind::Binary => "binary",
        BuildKind::Source => "source",
    };
    environment.root_dir.join(format!("manifest-{kind}.json"))
}

/// Selects which files from the source directory are staged into the build tree.
#[derive(
    Debug,
    Default,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    ValueEnum,
    serde::Deserialize,
    serde::Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum SourceSyncMode {
    /// Git-tracked files, including uncommitted modifications. Untracked
    /// files are not staged and reported as a warning.
    #[default]
    Tracked,
    /// Like `tracked`, but the build fails if the worktree has uncommitted
    /// changes or untracked files.
    Committed,
    /// All files except git-ignored ones, regardless of git tracking state.
    Worktree,
    /// All files except including git-ignored ones
    Everything,
}

/// Paths of files tracked by git in `src`, as reported by `git ls-files`.
/// Returns `None` if `src` is not inside a git worktree.
fn git_tracked_paths(src: &Path) -> anyhow::Result<Option<Vec<PathBuf>>> {
    let output = Command::new("git")
        .args(["-C"])
        .arg(src)
        .args(["ls-files", "-z"])
        .output()
        .context("failed to run git ls-files")?;
    if !output.status.success() {
        return Ok(None);
    }
    let mut paths = Vec::new();
    for raw in output.stdout.split(|byte| *byte == 0) {
        if raw.is_empty() {
            continue;
        }
        let path = PathBuf::from(OsStr::from_bytes(raw));
        validate_source_path(&path)?;
        paths.push(path);
    }
    Ok(Some(paths))
}

/// Paths of files git knows about but does not track (respecting ignore
/// rules), for warning about what a `tracked` sync leaves out.
fn git_untracked_paths(src: &Path) -> Vec<PathBuf> {
    let output = Command::new("git")
        .args(["-C"])
        .arg(src)
        .args(["ls-files", "-z", "--others", "--exclude-standard"])
        .output();
    match output {
        Ok(output) if output.status.success() => output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|raw| !raw.is_empty())
            .map(|raw| PathBuf::from(OsStr::from_bytes(raw)))
            .collect(),
        _ => Vec::new(),
    }
}

/// Ensure the git worktree in `src` has no uncommitted changes and no
/// untracked files, as required by `SourceSyncMode::Committed`.
fn git_ensure_clean_worktree(src: &Path) -> anyhow::Result<()> {
    let output = Command::new("git")
        .args(["-C"])
        .arg(src)
        .args(["status", "--porcelain"])
        .output()
        .context("failed to run git status")?;
    if !output.status.success() {
        // Not a git worktree; the caller falls back to worktree staging.
        return Ok(());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let entries: Vec<&str> = stdout.lines().filter(|line| !line.is_empty()).collect();
    if entries.is_empty() {
        return Ok(());
    }
    let mut message =
        String::from("source-sync mode 'committed' requires a clean git worktree, but found:\n");
    for entry in entries.iter().take(20) {
        message.push_str(&format!("  {entry}\n"));
    }
    if entries.len() > 20 {
        message.push_str(&format!("  ... and {} more\n", entries.len() - 20));
    }
    message.push_str("commit the changes or use a different --source-sync mode");
    bail!(message)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum SourcePathKind {
    Directory,
    File,
    Symlink,
}

/// Host paths as Unix bytes so Latin-1 lintian fixtures round-trip through JSON.
mod unix_path {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    use serde::{Deserializer, Serialize, Serializer};

    pub fn serialize<S>(path: &Path, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        path.as_os_str().as_bytes().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Manifests written before paths became bytes hold plain strings; a
        // persistent Environment can still carry one.
        struct PathVisitor;

        impl<'de> serde::de::Visitor<'de> for PathVisitor {
            type Value = PathBuf;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a path as a string or a sequence of bytes")
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<PathBuf, E> {
                Ok(PathBuf::from(value))
            }

            fn visit_bytes<E: serde::de::Error>(self, value: &[u8]) -> Result<PathBuf, E> {
                Ok(PathBuf::from(OsStr::from_bytes(value)))
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<PathBuf, A::Error> {
                let mut bytes = Vec::new();
                while let Some(byte) = sequence.next_element::<u8>()? {
                    bytes.push(byte);
                }
                Ok(PathBuf::from(OsStr::from_bytes(&bytes)))
            }
        }

        deserializer.deserialize_any(PathVisitor)
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct SourcePath {
    #[serde(with = "unix_path")]
    path: PathBuf,
    kind: SourcePathKind,
}

fn validate_source_path(path: &Path) -> anyhow::Result<()> {
    if path.as_os_str().is_empty()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        bail!("invalid source manifest path: {}", path.display());
    }
    Ok(())
}

fn source_tree_entries(src: &Path, mode: SourceSyncMode) -> anyhow::Result<Vec<SourcePath>> {
    match mode {
        SourceSyncMode::Everything => directory_entries(src),
        SourceSyncMode::Worktree => worktree_entries(src),
        SourceSyncMode::Tracked | SourceSyncMode::Committed => {
            if mode == SourceSyncMode::Committed {
                git_ensure_clean_worktree(src)?;
            }
            match git_tracked_paths(src)? {
                Some(paths) => tracked_entries(src, &paths),
                None => {
                    eprintln!(
                        "debmagic: warning: {} is not a git worktree, falling back to 'worktree' source sync",
                        src.display()
                    );
                    worktree_entries(src)
                }
            }
        }
    }
}

/// `output_dir` relative to `src`, when it lies inside it and is debmagic's.
fn owned_output_dir(src: &Path, output_dir: &Path) -> Option<PathBuf> {
    output_dir
        .strip_prefix(src)
        .ok()
        .filter(|relative| {
            !relative.as_os_str().is_empty() && crate::output_dir::is_tagged(output_dir)
        })
        .map(Path::to_path_buf)
}

/// The entries to stage: the sync mode's selection, minus debmagic's
/// output dir.
fn staged_entries(
    src: &Path,
    mode: SourceSyncMode,
    output_dir: &Path,
) -> anyhow::Result<Vec<SourcePath>> {
    let entries = source_tree_entries(src, mode)?;
    let Some(output) = owned_output_dir(src, output_dir) else {
        return Ok(entries);
    };
    Ok(entries
        .into_iter()
        .filter(|entry| !entry.path.starts_with(&output))
        .collect())
}

fn entry_kind(path: &Path) -> anyhow::Result<SourcePathKind> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to stat source path {}", path.display()))?;
    if metadata.is_dir() {
        Ok(SourcePathKind::Directory)
    } else if metadata.is_file() {
        Ok(SourcePathKind::File)
    } else if metadata.is_symlink() {
        Ok(SourcePathKind::Symlink)
    } else {
        Err(anyhow!(
            "unsupported file type in source tree: {}",
            path.display()
        ))
    }
}

/// Build the entry list from git-tracked paths: tracked files (plus their
/// parent directories), with submodule gitlinks skipped.
fn tracked_entries(src: &Path, paths: &[PathBuf]) -> anyhow::Result<Vec<SourcePath>> {
    let mut seen = std::collections::HashSet::new();
    let mut entries = Vec::new();
    for path in paths {
        // Add parent directories first.
        let mut ancestors: Vec<&Path> = path.ancestors().skip(1).collect();
        ancestors.pop(); // drop the empty "" ancestor
        ancestors.reverse();
        for ancestor in ancestors {
            if seen.insert(ancestor.to_path_buf()) {
                entries.push(SourcePath {
                    path: ancestor.to_path_buf(),
                    kind: SourcePathKind::Directory,
                });
            }
        }
        let full_path = src.join(path);
        // Submodule gitlinks are directories; their contents are not staged
        // since git does not track them as part of this repository.
        if full_path.is_dir() {
            eprintln!(
                "debmagic: skipping git submodule {}; its contents are not staged",
                path.display()
            );
            continue;
        }
        // A file deleted from the worktree but still in the index (no
        // `git rm`) has nothing to stage; the manifest diff removes it from
        // the build tree.
        if !full_path.exists() {
            eprintln!(
                "debmagic: warning: tracked file {} is missing from the worktree, building without it",
                path.display()
            );
            continue;
        }
        entries.push(SourcePath {
            path: path.clone(),
            kind: entry_kind(&full_path)?,
        });
    }
    entries.sort_by_key(|entry| entry.path.components().count());
    Ok(entries)
}

fn directory_entries(src: &Path) -> anyhow::Result<Vec<SourcePath>> {
    let walker = ignore::WalkBuilder::new(src)
        .standard_filters(false)
        .hidden(false)
        .build();

    let mut entries = Vec::new();
    for entry in walker {
        let entry = entry?;
        let relative_path = entry
            .path()
            .strip_prefix(src)
            .context("failed to get relative path")?;
        if relative_path.as_os_str().is_empty() {
            continue;
        }
        entries.push(SourcePath {
            path: relative_path.to_path_buf(),
            kind: entry_kind(entry.path())?,
        });
    }
    entries.sort_by_key(|entry| entry.path.components().count());
    Ok(entries)
}

fn worktree_entries(src: &Path) -> anyhow::Result<Vec<SourcePath>> {
    let walker = ignore::WalkBuilder::new(src)
        .standard_filters(true)
        .hidden(false)
        .filter_entry(|entry| !(entry.path().is_dir() && entry.path().ends_with(".git")))
        .build();

    let mut entries = Vec::new();
    for entry in walker {
        let entry = entry?;
        let relative_path = entry
            .path()
            .strip_prefix(src)
            .context("failed to get relative path")?;
        if relative_path.as_os_str().is_empty() {
            continue;
        }
        entries.push(SourcePath {
            path: relative_path.to_path_buf(),
            kind: entry_kind(entry.path())?,
        });
    }
    entries.sort_by_key(|entry| entry.path.components().count());
    Ok(entries)
}

fn remove_path(path: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn files_match(source: &Path, destination: &Path) -> std::io::Result<bool> {
    let source_metadata = fs::metadata(source)?;
    let destination_metadata = match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => return Ok(false),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if source_metadata.len() != destination_metadata.len()
        || source_metadata.permissions().mode() != destination_metadata.permissions().mode()
    {
        return Ok(false);
    }

    let mut source = BufReader::new(fs::File::open(source)?);
    let mut destination = BufReader::new(fs::File::open(destination)?);
    let mut source_buffer = [0; 8192];
    let mut destination_buffer = [0; 8192];
    loop {
        let source_len = source.read(&mut source_buffer)?;
        let destination_len = destination.read(&mut destination_buffer)?;
        if source_len != destination_len
            || source_buffer[..source_len] != destination_buffer[..destination_len]
        {
            return Ok(false);
        }
        if source_len == 0 {
            return Ok(true);
        }
    }
}

fn copy_source_entries(src: &Path, dst: &Path, entries: &[SourcePath]) -> anyhow::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in entries {
        let source = src.join(&entry.path);
        let destination = dst.join(&entry.path);
        match entry.kind {
            SourcePathKind::Directory => match fs::symlink_metadata(&destination) {
                Ok(metadata) if metadata.is_dir() => {}
                Ok(_) => {
                    remove_path(&destination)?;
                    fs::create_dir_all(&destination)?;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    fs::create_dir_all(&destination)?;
                }
                Err(error) => return Err(error.into()),
            },
            SourcePathKind::File => {
                if !files_match(&source, &destination)? {
                    remove_path(&destination)?;
                    fs::copy(&source, &destination)
                        .with_context(|| format!("failed to copy file: {}", source.display()))?;
                }
            }
            SourcePathKind::Symlink => {
                let target = fs::read_link(&source)?;
                if fs::read_link(&destination).ok().as_deref() != Some(target.as_path()) {
                    remove_path(&destination)?;
                    symlink(target, &destination)?;
                }
            }
        }
    }
    Ok(())
}

fn copy_glob(src_dir: &Path, pattern: &str, dest_dir: &Path) -> anyhow::Result<()> {
    let full_pattern = src_dir.join(pattern).to_string_lossy().into_owned();
    for entry in glob(&full_pattern)? {
        let path = entry?;
        if path.is_file() {
            let filename = path.file_name().ok_or(anyhow!(
                "Could not retrieve filename from {}",
                path.display()
            ))?;
            link_or_copy(&path, &dest_dir.join(filename))?;
        }
    }
    Ok(())
}

fn write_source_manifest(
    environment: &Environment,
    kind: BuildKind,
    entries: &[SourcePath],
) -> anyhow::Result<()> {
    let manifest_path = source_manifest_path(environment, kind);
    let temporary_path = manifest_path.with_extension("json.tmp");
    fs::write(&temporary_path, serde_json::to_vec_pretty(entries)?)?;
    fs::rename(temporary_path, manifest_path)?;
    Ok(())
}

fn sync_source_tree(
    environment: &Environment,
    kind: BuildKind,
    source_dir: &Path,
    source_sync_mode: SourceSyncMode,
    output_dir: &Path,
) -> anyhow::Result<()> {
    let manifest_path = source_manifest_path(environment, kind);
    let previous: Vec<SourcePath> = serde_json::from_reader(BufReader::new(
        fs::File::open(&manifest_path)
            .with_context(|| format!("failed to open {}", manifest_path.display()))?,
    ))
    .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    for entry in &previous {
        validate_source_path(&entry.path)?;
    }
    let current = staged_entries(source_dir, source_sync_mode, output_dir)?;

    let current_kinds = current
        .iter()
        .map(|entry| (entry.path.as_path(), entry.kind))
        .collect::<std::collections::HashMap<_, _>>();
    let mut stale = previous
        .iter()
        .filter(|entry| current_kinds.get(entry.path.as_path()) != Some(&entry.kind))
        .collect::<Vec<_>>();
    stale.sort_by_key(|entry| Reverse(entry.path.components().count()));
    for entry in stale {
        let destination = stage_dir(environment, kind).join(&entry.path);
        if entry.kind == SourcePathKind::Directory
            && !current_kinds.contains_key(entry.path.as_path())
        {
            match fs::remove_dir(&destination) {
                Ok(()) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::DirectoryNotEmpty
                    ) => {}
                Err(error) => return Err(error.into()),
            }
        } else {
            remove_path(&destination)?;
        }
    }

    copy_source_entries(source_dir, &stage_dir(environment, kind), &current)?;
    write_source_manifest(environment, kind, &current)
}

pub fn stage_source_tree(
    environment: &Environment,
    kind: BuildKind,
    package: &SourcePackage,
    source_sync_mode: SourceSyncMode,
    incremental: bool,
    output_dir: &Path,
) -> anyhow::Result<()> {
    let source_dir = package.source_dir()?;
    if source_sync_mode == SourceSyncMode::Tracked {
        // the artifacts of earlier builds are not the user's to `git add`
        let output = owned_output_dir(source_dir, output_dir);
        let untracked: Vec<PathBuf> = git_untracked_paths(source_dir)
            .into_iter()
            .filter(|path| {
                output
                    .as_ref()
                    .is_none_or(|output| !path.starts_with(output))
            })
            .collect();
        if !untracked.is_empty() {
            eprintln!(
                "debmagic: warning: {} untracked file(s) not staged into the build tree:",
                untracked.len()
            );
            for path in untracked.iter().take(20) {
                eprintln!("  {}", path.display());
            }
            if untracked.len() > 20 {
                eprintln!("  ... and {} more", untracked.len() - 20);
            }
            eprintln!("  `git add` them or use `--source-sync worktree` to include them");
        }
    }
    if incremental && source_manifest_path(environment, kind).is_file() {
        sync_source_tree(environment, kind, source_dir, source_sync_mode, output_dir)
            .context("failed to synchronize source tree")?;
    } else {
        let entries = staged_entries(source_dir, source_sync_mode, output_dir)?;
        copy_source_entries(source_dir, &stage_dir(environment, kind), &entries)
            .context("failed to copy source tree to build directory")?;
        write_source_manifest(environment, kind, &entries)?;
    }

    let source_parent = source_dir
        .parent()
        .ok_or_else(|| anyhow!("source directory has no parent"))?;
    // the naming knowledge lives in debmagic-common; only the glob
    // wildcard for "any compression" is added here
    let orig_pattern = format!(
        "{}*",
        debmagic_common::artifact::orig_prefix(
            package.name(),
            package.version().upstream_version()
        )
    );
    let component_pattern = format!(
        "{}*",
        debmagic_common::artifact::component_orig_prefix(
            package.name(),
            package.version().upstream_version(),
            "*"
        )
    );
    copy_glob(source_parent, &orig_pattern, &environment.work_dir())?;
    copy_glob(source_parent, &component_pattern, &environment.work_dir())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt;

    use crate::driver::{DriverType, Environment, EnvironmentPurpose, Persistence};

    use super::*;

    #[test]
    fn incremental_sync_updates_sources_and_preserves_build_outputs() -> anyhow::Result<()> {
        let _test_root_tmp = tempfile::Builder::new()
            .prefix("debmagic-incremental-test-")
            .tempdir()
            .unwrap();
        let test_root = _test_root_tmp.path().to_path_buf();
        let source_dir = test_root.join("source");
        let build_root_dir = test_root.join("build");
        fs::create_dir_all(source_dir.join("cache"))?;
        fs::write(source_dir.join("changed.txt"), "before")?;
        fs::write(source_dir.join("unchanged.txt"), "unchanged")?;
        fs::write(source_dir.join("removed.txt"), "remove me")?;
        fs::write(source_dir.join("cache/input.c"), "source")?;
        symlink("changed.txt", source_dir.join("link"))?;

        let environment = Environment {
            driver: DriverType::Bare,
            package_name: "example".to_string(),
            package_identifier: "example-1.0".to_string(),
            source_dir: source_dir.clone(),
            root_dir: build_root_dir.clone(),
            distro: debmagic_common::distro::DistroVersion::new(
                debmagic_common::distro::Distro::Debian,
                "trixie",
                "13",
            ),
            persistence: Persistence::Always,
            purpose: EnvironmentPurpose::Build,
        };
        environment.create_dirs()?;
        let initial_entries = source_tree_entries(&source_dir, SourceSyncMode::Worktree)?;
        copy_source_entries(
            &source_dir,
            &stage_dir(&environment, BuildKind::Binary),
            &initial_entries,
        )?;
        write_source_manifest(&environment, BuildKind::Binary, &initial_entries)?;
        let unchanged_inode =
            fs::metadata(stage_dir(&environment, BuildKind::Binary).join("unchanged.txt"))?.ino();
        fs::write(
            stage_dir(&environment, BuildKind::Binary).join("cache/output.o"),
            "compiled",
        )?;

        fs::write(source_dir.join("changed.txt"), "after")?;
        fs::remove_file(source_dir.join("removed.txt"))?;
        fs::remove_file(source_dir.join("cache/input.c"))?;
        fs::remove_dir(source_dir.join("cache"))?;
        fs::remove_file(source_dir.join("link"))?;
        symlink("added.txt", source_dir.join("link"))?;
        fs::write(source_dir.join("added.txt"), "new")?;

        sync_source_tree(
            &environment,
            BuildKind::Binary,
            &source_dir,
            SourceSyncMode::Worktree,
            &source_dir.join("build"),
        )?;

        let staged = stage_dir(&environment, BuildKind::Binary);
        assert_eq!(fs::read_to_string(staged.join("changed.txt"))?, "after");
        assert_eq!(fs::read_to_string(staged.join("added.txt"))?, "new");
        assert_eq!(
            fs::metadata(staged.join("unchanged.txt"))?.ino(),
            unchanged_inode
        );
        assert_eq!(fs::read_link(staged.join("link"))?, Path::new("added.txt"));
        assert!(!staged.join("removed.txt").exists());
        assert!(!staged.join("cache/input.c").exists());
        assert_eq!(
            fs::read_to_string(staged.join("cache/output.o"))?,
            "compiled"
        );

        Ok(())
    }

    /// The orig tarball in `..` symlinks to a file already hardlinked into
    /// the work dir: re-staging it must not truncate the shared inode.
    #[test]
    fn copy_glob_keeps_hardlinked_destination() -> anyhow::Result<()> {
        let _test_root_tmp = tempfile::Builder::new()
            .prefix("debmagic-copy-glob-test-")
            .tempdir()
            .unwrap();
        let test_root = _test_root_tmp.path().to_path_buf();
        let parent = test_root.join("parent");
        let work = test_root.join("work");
        fs::create_dir_all(&parent)?;
        fs::create_dir_all(&work)?;
        let cached = test_root.join("cached.orig.tar.xz");
        fs::write(&cached, "tarball")?;
        symlink(&cached, parent.join("pkg_1.0.orig.tar.xz"))?;
        fs::hard_link(&cached, work.join("pkg_1.0.orig.tar.xz"))?;

        copy_glob(&parent, "pkg_1.0.orig.tar*", &work)?;

        assert_eq!(fs::read_to_string(&cached)?, "tarball");
        assert_eq!(
            fs::read_to_string(work.join("pkg_1.0.orig.tar.xz"))?,
            "tarball"
        );
        Ok(())
    }

    #[test]
    fn source_manifest_reads_string_and_byte_paths() {
        let entries: Vec<SourcePath> = serde_json::from_str(
            r#"[{"path": ".bumpversion.toml", "kind": "file"},
                {"path": [100, 101, 98, 105, 97, 110], "kind": "directory"}]"#,
        )
        .expect("both path forms load");
        assert_eq!(entries[0].path, PathBuf::from(".bumpversion.toml"));
        assert_eq!(entries[1].path, PathBuf::from("debian"));
        assert_eq!(entries[1].kind, SourcePathKind::Directory);
    }

    #[test]
    fn source_manifest_paths_must_be_relative_and_normal() {
        assert!(validate_source_path(Path::new("debian/control")).is_ok());
        for path in ["", ".", "../outside", "debian/../outside", "/tmp/outside"] {
            assert!(validate_source_path(Path::new(path)).is_err(), "{path}");
        }
    }

    #[test]
    fn manifests_and_stage_dirs_are_per_build_kind() {
        let environment = Environment {
            driver: DriverType::Bare,
            package_name: "example".to_string(),
            package_identifier: "example-1.0".to_string(),
            source_dir: PathBuf::from("/src"),
            root_dir: PathBuf::from("/tmp/debmagic/example-1.0"),
            distro: debmagic_common::distro::DistroVersion::new(
                debmagic_common::distro::Distro::Debian,
                "trixie",
                "13",
            ),
            persistence: Persistence::Always,
            purpose: EnvironmentPurpose::Build,
        };

        assert_eq!(
            source_manifest_path(&environment, BuildKind::Binary),
            PathBuf::from("/tmp/debmagic/example-1.0/manifest-binary.json")
        );
        assert_eq!(
            source_manifest_path(&environment, BuildKind::Source),
            PathBuf::from("/tmp/debmagic/example-1.0/manifest-source.json")
        );
        assert_eq!(
            stage_dir(&environment, BuildKind::Binary),
            PathBuf::from("/tmp/debmagic/example-1.0/work/example-1.0-binary")
        );
        assert_eq!(
            stage_dir(&environment, BuildKind::Source),
            PathBuf::from("/tmp/debmagic/example-1.0/work/example-1.0-source")
        );
    }

    /// A `SourcePackage` whose `Location::SourceDir` points at `dir`.
    fn test_package(dir: &Path) -> SourcePackage {
        crate::package::load_package(dir).unwrap()
    }

    /// A source build sharing a build root with a binary build must never
    /// touch the binary tree or its manifest: it stages into its own tree
    /// and syncs against its own manifest.
    #[test]
    fn source_stage_sync_leaves_binary_tree_untouched() -> anyhow::Result<()> {
        let _test_root_tmp = tempfile::Builder::new()
            .prefix("debmagic-source-stage-test-")
            .tempdir()
            .unwrap();
        let test_root = _test_root_tmp.path().to_path_buf();
        let source_dir = test_root.join("source");
        fs::create_dir_all(source_dir.join("debian"))?;
        fs::write(
            source_dir.join("debian/changelog"),
            "example (1.0-1) unstable; urgency=medium\n\n  * Change.\n\n -- A <a@example.com>  Mon, 01 Jan 2024 00:00:00 +0000\n",
        )?;
        fs::write(source_dir.join("README"), "read me")?;

        let environment = Environment {
            driver: DriverType::Bare,
            package_name: "example".to_string(),
            package_identifier: "example-1.0".to_string(),
            source_dir: source_dir.clone(),
            root_dir: test_root.join("build"),
            distro: debmagic_common::distro::DistroVersion::new(
                debmagic_common::distro::Distro::Debian,
                "trixie",
                "13",
            ),
            persistence: Persistence::Always,
            purpose: EnvironmentPurpose::Build,
        };
        environment.create_dirs()?;

        // a previous binary build staged the tree and left an output behind
        let package = test_package(&source_dir);
        stage_source_tree(
            &environment,
            BuildKind::Binary,
            &package,
            SourceSyncMode::Worktree,
            false,
            &source_dir.join("build"),
        )?;
        fs::write(
            stage_dir(&environment, BuildKind::Binary).join("compiled.o"),
            "binary build output",
        )?;
        let binary_manifest =
            fs::read_to_string(source_manifest_path(&environment, BuildKind::Binary))?;

        // the source build full-stages into its own tree
        stage_source_tree(
            &environment,
            BuildKind::Source,
            &package,
            SourceSyncMode::Worktree,
            false,
            &source_dir.join("build"),
        )?;
        assert!(
            stage_dir(&environment, BuildKind::Source)
                .join("debian/changelog")
                .is_file()
        );
        assert!(
            !stage_dir(&environment, BuildKind::Source)
                .join("compiled.o")
                .exists()
        );

        // an incremental source sync updates only the source tree
        fs::write(source_dir.join("README"), "changed")?;
        stage_source_tree(
            &environment,
            BuildKind::Source,
            &package,
            SourceSyncMode::Worktree,
            true,
            &source_dir.join("build"),
        )?;
        assert_eq!(
            fs::read_to_string(stage_dir(&environment, BuildKind::Source).join("README"))?,
            "changed"
        );

        // the binary tree and its manifest are untouched throughout
        assert_eq!(
            fs::read_to_string(stage_dir(&environment, BuildKind::Binary).join("README"))?,
            "read me"
        );
        assert!(
            stage_dir(&environment, BuildKind::Binary)
                .join("compiled.o")
                .is_file()
        );
        assert_eq!(
            fs::read_to_string(source_manifest_path(&environment, BuildKind::Binary))?,
            binary_manifest
        );

        Ok(())
    }

    /// Create a git repo with one committed file in a fresh temp dir.
    fn git_test_repo() -> anyhow::Result<tempfile::TempDir> {
        let tmp = tempfile::Builder::new()
            .prefix("debmagic-git-test-")
            .tempdir()?;
        let repo = tmp.path().to_path_buf();
        fs::create_dir_all(repo.join("debian"))?;
        fs::write(repo.join("debian/control"), "Source: example")?;
        let run = |args: &[&str]| -> anyhow::Result<()> {
            let status = Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(args)
                .status()?;
            if status.success() {
                Ok(())
            } else {
                Err(anyhow!("git {:?} failed", args))
            }
        };
        run(&["init", "-q"])?;
        run(&["add", "debian/control"])?;
        run(&[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-qm",
            "init",
        ])?;
        Ok(tmp)
    }

    #[test]
    fn tracked_sync_stages_only_git_tracked_files() -> anyhow::Result<()> {
        let _repo_tmp = git_test_repo()?;
        let repo = _repo_tmp.path().to_path_buf();
        fs::write(repo.join("untracked.txt"), "not staged")?;
        fs::write(repo.join("dirty.txt"), "uncommitted but tracked? no")?;
        // A tracked file with uncommitted modifications is staged with its
        // worktree content.
        fs::write(repo.join("debian/rules"), "new content")?;
        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["add", "debian/rules"])
            .status()?;

        let entries = source_tree_entries(&repo, SourceSyncMode::Tracked)?;
        let paths: Vec<&Path> = entries.iter().map(|e| e.path.as_path()).collect();
        assert!(paths.contains(&Path::new("debian")));
        assert!(paths.contains(&Path::new("debian/control")));
        assert!(paths.contains(&Path::new("debian/rules")));
        assert!(!paths.contains(&Path::new("untracked.txt")));
        assert!(!paths.contains(&Path::new("dirty.txt")));
        assert_eq!(git_untracked_paths(&repo).len(), 2);

        Ok(())
    }

    #[test]
    fn staging_skips_the_tagged_output_dir_only() -> anyhow::Result<()> {
        let _dir_tmp = tempfile::Builder::new()
            .prefix("debmagic-output-")
            .tempdir()
            .unwrap();
        let dir = _dir_tmp.path().to_path_buf();
        // upstream's build/ next to debmagic's output dir
        fs::create_dir_all(dir.join("build"))?;
        fs::write(dir.join("build/config.sub"), "upstream")?;
        crate::output_dir::create(&dir.join("build-debmagic"))?;
        fs::write(dir.join("build-debmagic/pkg_1.0_amd64.deb"), "artifact")?;

        let entries = staged_entries(&dir, SourceSyncMode::Worktree, &dir.join("build-debmagic"))?;
        let paths: Vec<&Path> = entries.iter().map(|e| e.path.as_path()).collect();
        assert!(paths.contains(&Path::new("build/config.sub")));
        assert!(!paths.iter().any(|path| path.starts_with("build-debmagic")));

        // -o pointing at upstream's untagged dir stages it
        let entries = staged_entries(&dir, SourceSyncMode::Worktree, &dir.join("build"))?;
        assert!(
            entries
                .iter()
                .any(|e| e.path == Path::new("build/config.sub"))
        );

        Ok(())
    }

    #[test]
    fn committed_sync_requires_clean_worktree() -> anyhow::Result<()> {
        let _repo_tmp = git_test_repo()?;
        let repo = _repo_tmp.path().to_path_buf();
        assert!(source_tree_entries(&repo, SourceSyncMode::Committed).is_ok());

        fs::write(repo.join("untracked.txt"), "dirty")?;
        let result = source_tree_entries(&repo, SourceSyncMode::Committed);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("requires a clean git worktree")
        );

        Ok(())
    }

    #[test]
    fn tracked_sync_skips_files_deleted_from_worktree() -> anyhow::Result<()> {
        let _repo_tmp = git_test_repo()?;
        let repo = _repo_tmp.path().to_path_buf();
        fs::remove_file(repo.join("debian/control"))?;

        let entries = source_tree_entries(&repo, SourceSyncMode::Tracked)?;
        let paths: Vec<&Path> = entries.iter().map(|e| e.path.as_path()).collect();
        assert!(!paths.contains(&Path::new("debian/control")));
        assert!(paths.contains(&Path::new("debian")));

        Ok(())
    }

    #[test]
    fn tracked_sync_stages_git_paths_that_are_not_utf8() -> anyhow::Result<()> {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let repo_tmp = git_test_repo()?;
        let repo = repo_tmp.path();
        let name = OsStr::from_bytes(b"bokm\xe5l");
        fs::write(repo.join(name), "latin-1 filename")?;
        let status = Command::new("git")
            .arg("-C")
            .arg(repo)
            .arg("add")
            .arg("--")
            .arg(name)
            .status()?;
        assert!(status.success());

        let entries = source_tree_entries(repo, SourceSyncMode::Tracked)?;
        assert!(
            entries.iter().any(|entry| entry.path.as_os_str() == name),
            "tracked entries: {:?}",
            entries
                .iter()
                .map(|entry| entry.path.as_os_str().as_bytes().to_vec())
                .collect::<Vec<_>>()
        );

        let test_root =
            std::env::temp_dir().join(format!("debmagic-latin1-stage-{}", uuid::Uuid::new_v4()));
        let environment = Environment {
            driver: DriverType::Bare,
            package_name: "example".to_string(),
            package_identifier: "example-1.0".to_string(),
            source_dir: repo.to_path_buf(),
            root_dir: test_root.clone(),
            distro: debmagic_common::distro::DistroVersion::new(
                debmagic_common::distro::Distro::Debian,
                "trixie",
                "13",
            ),
            persistence: Persistence::Always,
            purpose: EnvironmentPurpose::Build,
        };
        environment.create_dirs()?;
        let staged = stage_dir(&environment, BuildKind::Source);
        copy_source_entries(repo, &staged, &entries)?;
        write_source_manifest(&environment, BuildKind::Source, &entries)?;
        let loaded: Vec<SourcePath> = serde_json::from_reader(BufReader::new(fs::File::open(
            source_manifest_path(&environment, BuildKind::Source),
        )?))?;
        assert!(loaded.iter().any(|entry| entry.path.as_os_str() == name));
        assert_eq!(fs::read(staged.join(name))?, b"latin-1 filename");

        fs::remove_dir_all(test_root)?;
        Ok(())
    }

    #[test]
    fn tracked_sync_falls_back_outside_git_worktree() -> anyhow::Result<()> {
        let _dir_tmp = tempfile::Builder::new()
            .prefix("debmagic-nogit-")
            .tempdir()
            .unwrap();
        let dir = _dir_tmp.path().to_path_buf();
        fs::create_dir_all(&dir)?;
        fs::write(dir.join("file.txt"), "content")?;
        let entries = source_tree_entries(&dir, SourceSyncMode::Tracked)?;
        assert!(entries.iter().any(|e| e.path == Path::new("file.txt")));
        Ok(())
    }
}
