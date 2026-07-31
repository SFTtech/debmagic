//! Staging of the source tree into the build directory.
//!
//! Which files are staged is selected by `SourceSyncMode`: git-tracked
//! files (optionally requiring a clean worktree) or everything that isn't
//! git-ignored. Staging copies the selected entries, preserving symlinks
//! and keeping unchanged files untouched so incremental builds keep their
//! outputs; a manifest of staged entries lets incremental syncs remove
//! stale paths.

use std::cmp::Reverse;
use std::io::{self, BufReader, Read};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Component, Path, PathBuf};
use std::{fs, process::Command};

use anyhow::{Context, anyhow, bail};
use glob::glob;

use crate::build::common::{BuildConfig, SourceSyncMode};
use crate::package::PackageDescription;

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
        let path = PathBuf::from(String::from_utf8(raw.to_vec()).with_context(|| {
            format!("git-tracked path is not valid UTF-8 in {}", src.display())
        })?);
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
            .filter_map(|raw| String::from_utf8(raw.to_vec()).ok())
            .map(PathBuf::from)
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

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct SourcePath {
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
        entries.push(SourcePath {
            path: path.clone(),
            kind: entry_kind(&full_path)?,
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
            fs::copy(&path, dest_dir.join(filename))?;
        }
    }
    Ok(())
}

pub fn source_manifest_path(build_config: &BuildConfig) -> PathBuf {
    build_config.build_root_dir.join("source-manifest.json")
}

fn write_source_manifest(build_config: &BuildConfig, entries: &[SourcePath]) -> anyhow::Result<()> {
    let manifest_path = source_manifest_path(build_config);
    let temporary_path = manifest_path.with_extension("json.tmp");
    fs::write(&temporary_path, serde_json::to_vec_pretty(entries)?)?;
    fs::rename(temporary_path, manifest_path)?;
    Ok(())
}

fn sync_source_tree(build_config: &BuildConfig) -> anyhow::Result<()> {
    let manifest_path = source_manifest_path(build_config);
    let previous: Vec<SourcePath> = serde_json::from_reader(BufReader::new(
        fs::File::open(&manifest_path)
            .with_context(|| format!("failed to open {}", manifest_path.display()))?,
    ))
    .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    for entry in &previous {
        validate_source_path(&entry.path)?;
    }
    let current = source_tree_entries(&build_config.source_dir, build_config.source_sync_mode)?;

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
        let destination = build_config.build_source_dir().join(&entry.path);
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

    copy_source_entries(
        &build_config.source_dir,
        &build_config.build_source_dir(),
        &current,
    )?;
    write_source_manifest(build_config, &current)
}

pub fn stage_source_tree(
    build_config: &BuildConfig,
    package: &PackageDescription,
) -> anyhow::Result<()> {
    if build_config.source_sync_mode == SourceSyncMode::Tracked {
        let untracked = git_untracked_paths(&build_config.source_dir);
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
            eprintln!("  git add them or use --source-sync worktree to include them");
        }
    }
    if build_config.incremental && source_manifest_path(build_config).is_file() {
        sync_source_tree(build_config).context("failed to synchronize source tree")?;
    } else {
        let entries = source_tree_entries(&build_config.source_dir, build_config.source_sync_mode)?;
        copy_source_entries(
            &build_config.source_dir,
            &build_config.build_source_dir(),
            &entries,
        )
        .context("failed to copy source tree to build directory")?;
        write_source_manifest(build_config, &entries)?;
    }

    let source_parent = build_config
        .source_dir
        .parent()
        .ok_or_else(|| anyhow!("source directory has no parent"))?;
    let prefix = format!("{}_{}", package.name, package.version.upstream_version());
    copy_glob(
        source_parent,
        &format!("{prefix}.orig.tar.*"),
        &build_config.build_work_dir(),
    )?;
    copy_glob(
        source_parent,
        &format!("{prefix}.orig-*.tar.*"),
        &build_config.build_work_dir(),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt;

    use crate::build::common::BuildDriverType;

    use super::*;

    #[test]
    fn incremental_sync_updates_sources_and_preserves_build_outputs() -> anyhow::Result<()> {
        let test_root = std::env::temp_dir().join(format!(
            "debmagic-incremental-test-{}",
            uuid::Uuid::new_v4()
        ));
        let source_dir = test_root.join("source");
        let build_root_dir = test_root.join("build");
        fs::create_dir_all(source_dir.join("cache"))?;
        fs::write(source_dir.join("changed.txt"), "before")?;
        fs::write(source_dir.join("unchanged.txt"), "unchanged")?;
        fs::write(source_dir.join("removed.txt"), "remove me")?;
        fs::write(source_dir.join("cache/input.c"), "source")?;
        symlink("changed.txt", source_dir.join("link"))?;

        let build_config = BuildConfig {
            driver: BuildDriverType::Bare,
            package_name: "example".to_string(),
            package_identifier: "example-1.0".to_string(),
            build_root_dir: build_root_dir.clone(),
            source_dir: source_dir.clone(),
            output_dir: test_root.join("output"),
            distro: debmagic_common::distro::get_distro_version("trixie").unwrap(),
            sign_package: false,
            sign_key: None,
            build_debug_symbols: false,
            clean: false,
            persistent: true,
            incremental: true,
            source_sync_mode: SourceSyncMode::Worktree,
        };
        build_config.create_dirs()?;
        let initial_entries = source_tree_entries(&source_dir, SourceSyncMode::Worktree)?;
        copy_source_entries(
            &source_dir,
            &build_config.build_source_dir(),
            &initial_entries,
        )?;
        write_source_manifest(&build_config, &initial_entries)?;
        let unchanged_inode =
            fs::metadata(build_config.build_source_dir().join("unchanged.txt"))?.ino();
        fs::write(
            build_config.build_source_dir().join("cache/output.o"),
            "compiled",
        )?;

        fs::write(source_dir.join("changed.txt"), "after")?;
        fs::remove_file(source_dir.join("removed.txt"))?;
        fs::remove_file(source_dir.join("cache/input.c"))?;
        fs::remove_dir(source_dir.join("cache"))?;
        fs::remove_file(source_dir.join("link"))?;
        symlink("added.txt", source_dir.join("link"))?;
        fs::write(source_dir.join("added.txt"), "new")?;

        sync_source_tree(&build_config)?;

        let staged = build_config.build_source_dir();
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

        fs::remove_dir_all(test_root)?;
        Ok(())
    }

    #[test]
    fn source_manifest_paths_must_be_relative_and_normal() {
        assert!(validate_source_path(Path::new("debian/control")).is_ok());
        for path in ["", ".", "../outside", "debian/../outside", "/tmp/outside"] {
            assert!(validate_source_path(Path::new(path)).is_err(), "{path}");
        }
    }

    /// Create a git repo with one committed file in a fresh temp dir.
    fn git_test_repo() -> anyhow::Result<PathBuf> {
        let repo = std::env::temp_dir().join(format!("debmagic-git-test-{}", uuid::Uuid::new_v4()));
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
        Ok(repo)
    }

    #[test]
    fn tracked_sync_stages_only_git_tracked_files() -> anyhow::Result<()> {
        let repo = git_test_repo()?;
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

        fs::remove_dir_all(repo)?;
        Ok(())
    }

    #[test]
    fn committed_sync_requires_clean_worktree() -> anyhow::Result<()> {
        let repo = git_test_repo()?;
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

        fs::remove_dir_all(repo)?;
        Ok(())
    }

    #[test]
    fn tracked_sync_falls_back_outside_git_worktree() -> anyhow::Result<()> {
        let dir = std::env::temp_dir().join(format!("debmagic-nogit-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir)?;
        fs::write(dir.join("file.txt"), "content")?;
        let entries = source_tree_entries(&dir, SourceSyncMode::Tracked)?;
        assert!(entries.iter().any(|e| e.path == Path::new("file.txt")));
        fs::remove_dir_all(dir)?;
        Ok(())
    }
}
