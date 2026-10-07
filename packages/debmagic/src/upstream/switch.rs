use std::path::{Path, PathBuf};

use anyhow::Context;
use debmagic_common::package::SourcePackage;

use crate::upstream::fetch::{FetchOptions, Fetched, extract_tarball, fetch};
use crate::upstream::watch::WatchSource;

/// Switch the package tree to an upstream version: fetch its orig
/// tarball like `upstream fetch`, then replace the tree's contents with
/// the tarball's (keeping `debian/`). A component source swaps its
/// `<component>/` subdir instead; `components` are the package's MUT
/// component names, whose subdirs the main tree swap keeps.
pub async fn switch(
    package: &SourcePackage,
    source: &WatchSource,
    requested: &str,
    options: &FetchOptions<'_>,
    components: &[String],
) -> anyhow::Result<Fetched> {
    let source_dir = package.source_dir()?;
    let fetched = fetch(package, source, requested, options).await?;
    let target_dir = match &source.component {
        Some(component) => source_dir.join(component),
        None => source_dir.to_path_buf(),
    };
    if options.dry_run {
        println!(
            "debmagic: would replace {} with the contents of {}",
            target_dir.display(),
            fetched.tarball.display()
        );
        return Ok(fetched);
    }

    let work_dir = std::env::temp_dir().join(format!(
        "debmagic-switch-{}-{}",
        package.name(),
        std::process::id()
    ));
    let extract_dir = work_dir.join("tree");
    let swapped = extract_tarball(&fetched.tarball, &extract_dir).and_then(|()| {
        // a component lives in its own subdir, like dpkg-source unpacks
        // it; the main tree keeps the component subdirs and the output dir
        let mut keep: Vec<PathBuf> = match &source.component {
            Some(_) => Vec::new(),
            None => components
                .iter()
                .map(|component| source_dir.join(component))
                .collect(),
        };
        keep.push(options.output_dir.to_path_buf());
        std::fs::create_dir_all(&target_dir)
            .with_context(|| format!("failed to create {}", target_dir.display()))?;
        swap_tree(&target_dir, &extract_dir, &keep)
    });
    std::fs::remove_dir_all(&work_dir).ok();
    swapped.with_context(|| {
        format!(
            "replacing {} with {} failed",
            target_dir.display(),
            fetched.tarball.display()
        )
    })?;
    println!(
        "debmagic: switched {} to upstream {} (orig: {})",
        target_dir.display(),
        fetched.upstream_version,
        fetched.tarball.display()
    );
    Ok(fetched)
}

/// Move one entry, falling back to copy+remove when a rename crosses
/// filesystems (the work dir is in /tmp, the source tree may not be).
fn move_entry(from: &Path, to: &Path) -> anyhow::Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    if from.is_dir() {
        copy_dir(from, to)?;
        std::fs::remove_dir_all(from)
    } else {
        std::fs::copy(from, to)
            .map(|_| ())
            .and_then(|()| std::fs::remove_file(from))
    }
    .with_context(|| format!("failed to move {} to {}", from.display(), to.display()))
}

/// Recursively copy a directory. Symlinks are recreated as symlinks —
/// a tarball may contain dangling ones, and following them would fail.
fn copy_dir(from: &Path, to: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(to).with_context(|| format!("failed to create {}", to.display()))?;
    for entry in std::fs::read_dir(from)
        .with_context(|| format!("failed to read {}", from.display()))?
        .flatten()
    {
        let destination = to.join(entry.file_name());
        let path = entry.path();
        if entry.file_type()?.is_symlink() {
            let target = std::fs::read_link(&path)
                .with_context(|| format!("failed to read link {}", path.display()))?;
            // a leftover entry from an interrupted run must not block the swap
            if std::fs::symlink_metadata(&destination).is_ok() {
                if destination.is_dir() {
                    std::fs::remove_dir_all(&destination)
                        .with_context(|| format!("failed to remove {}", destination.display()))?;
                } else {
                    std::fs::remove_file(&destination)
                        .with_context(|| format!("failed to remove {}", destination.display()))?;
                }
            }
            std::os::unix::fs::symlink(&target, &destination).with_context(|| {
                format!(
                    "failed to create symlink {} -> {}",
                    destination.display(),
                    target.display()
                )
            })?;
        } else if path.is_dir() {
            copy_dir(&path, &destination)?;
        } else {
            std::fs::copy(&path, &destination).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    path.display(),
                    destination.display()
                )
            })?;
        }
    }
    Ok(())
}

/// Replace the source tree's contents with the extracted tree's,
/// keeping `debian/`, any VCS metadata dir (`.git`, ...) and every
/// `keep` path (or path containing one) intact.
fn swap_tree(source_dir: &Path, extract_dir: &Path, keep: &[PathBuf]) -> anyhow::Result<()> {
    let new_names: Vec<String> = std::fs::read_dir(extract_dir)
        .with_context(|| format!("failed to read {}", extract_dir.display()))?
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();

    // stale entries from the old tree that the new one doesn't have
    for entry in std::fs::read_dir(source_dir)
        .with_context(|| format!("failed to read {}", source_dir.display()))?
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if is_kept(&name)
            || new_names.contains(&name)
            || keep.iter().any(|kept| kept.starts_with(&path))
        {
            continue;
        }
        if path.is_dir() {
            std::fs::remove_dir_all(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        } else {
            std::fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }

    // move the new tree in
    for name in &new_names {
        if is_kept(name) {
            continue;
        }
        move_entry(&extract_dir.join(name), &source_dir.join(name))?;
    }
    Ok(())
}

/// Entries that always survive a tree swap: the packaging dir and
/// VCS storage (`.git`)
fn is_kept(name: &str) -> bool {
    name == "debian" || name == ".git"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_and_swap() {
        let base = std::env::temp_dir().join(format!("debmagic-switch-{}", std::process::id()));
        let source_dir = base.join("pkg");
        let extract_dir = base.join("tree");
        std::fs::create_dir_all(source_dir.join("debian")).unwrap();
        std::fs::create_dir_all(&extract_dir).unwrap();
        // old tree: a file that the new tree lacks, plus debian/
        std::fs::write(source_dir.join("old.txt"), "old").unwrap();
        std::fs::write(source_dir.join("debian").join("changelog"), "keep").unwrap();
        let output_dir = source_dir.join("build");
        std::fs::create_dir_all(&output_dir).unwrap();
        std::fs::write(output_dir.join("pkg_2.0.orig.tar.xz"), "orig").unwrap();
        // a MUT component unpacked into its subdir
        let component_dir = source_dir.join("plugin");
        std::fs::create_dir_all(&component_dir).unwrap();
        std::fs::write(component_dir.join("plugin.c"), "x").unwrap();
        // new tree
        std::fs::write(extract_dir.join("new.txt"), "new").unwrap();
        std::fs::create_dir_all(extract_dir.join("src")).unwrap();
        std::fs::write(extract_dir.join("src").join("main.c"), "x").unwrap();

        swap_tree(
            &source_dir,
            &extract_dir,
            &[output_dir.clone(), component_dir.clone()],
        )
        .unwrap();

        assert!(source_dir.join("new.txt").exists());
        assert!(source_dir.join("src").join("main.c").exists());
        assert!(!source_dir.join("old.txt").exists());
        assert!(source_dir.join("debian").join("changelog").exists());
        assert!(output_dir.join("pkg_2.0.orig.tar.xz").exists());
        assert!(component_dir.join("plugin.c").exists());
        std::fs::remove_dir_all(&base).unwrap();
    }
}
