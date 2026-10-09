use std::path::{Component, Path, PathBuf};

use anyhow::{Context, bail};
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
    let mut fetched = fetch(package, source, requested, options).await?;
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
        let output_dir = make_room_for_upstream(&target_dir, &extract_dir, options)?;
        keep.push(output_dir.clone());
        std::fs::create_dir_all(&target_dir)
            .with_context(|| format!("failed to create {}", target_dir.display()))?;
        swap_tree(&target_dir, &extract_dir, &keep, source.component.is_none())?;
        Ok(output_dir)
    });
    std::fs::remove_dir_all(&work_dir).ok();
    let output_dir = swapped.with_context(|| {
        format!(
            "replacing {} with {} failed",
            target_dir.display(),
            fetched.tarball.display()
        )
    })?;
    if let Ok(relative) = fetched.tarball.strip_prefix(options.output_dir) {
        fetched.tarball = output_dir.join(relative);
    }
    println!(
        "debmagic: switched {} to upstream {} (orig: {})",
        target_dir.display(),
        fetched.upstream_version,
        fetched.tarball.display()
    );
    Ok(fetched)
}

/// The output dir the swap must keep. When the tarball ships the top-level
/// entry holding it, the swap would merge upstream's files into the
/// artifacts: a debmagic-owned output dir moves to the fallback, anything
/// else fails.
fn make_room_for_upstream(
    target_dir: &Path,
    extract_dir: &Path,
    options: &FetchOptions,
) -> anyhow::Result<PathBuf> {
    let output_dir = options.output_dir;
    let Ok(relative) = output_dir.strip_prefix(target_dir) else {
        return Ok(output_dir.to_path_buf());
    };
    let Some(Component::Normal(top)) = relative.components().next() else {
        return Ok(output_dir.to_path_buf());
    };
    if std::fs::symlink_metadata(extract_dir.join(top)).is_err() {
        return Ok(output_dir.to_path_buf());
    }
    if let Some(fallback) = options.output_dir_fallback
        && target_dir.join(top) == output_dir
        && crate::output_dir::is_tagged(output_dir)
        && std::fs::symlink_metadata(fallback).is_err()
        && extract_dir
            .join(fallback.strip_prefix(target_dir).unwrap_or(fallback))
            .symlink_metadata()
            .is_err()
    {
        std::fs::rename(output_dir, fallback).with_context(|| {
            format!(
                "failed to move {} to {}",
                output_dir.display(),
                fallback.display()
            )
        })?;
        println!(
            "debmagic: upstream ships its own {}, moved the output dir to {}",
            top.to_string_lossy(),
            fallback.display()
        );
        return Ok(fallback.to_path_buf());
    }
    bail!(
        "the upstream tarball ships its own {}, which holds the output dir {}; \
         set `output_dir` or pass `-o` to use another directory",
        top.to_string_lossy(),
        output_dir.display()
    );
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
/// keeping any VCS metadata dir (`.git`, ...), `debian/` when
/// `main_tree` and every `keep` path (or path containing one) intact.
/// dpkg-source only replaces the main orig's `debian/`, a component's
/// is regular upstream content.
fn swap_tree(
    source_dir: &Path,
    extract_dir: &Path,
    keep: &[PathBuf],
    main_tree: bool,
) -> anyhow::Result<()> {
    let is_kept = |name: &str| name == ".git" || (main_tree && name == "debian");
    let new_names: Vec<String> = std::fs::read_dir(extract_dir)
        .with_context(|| format!("failed to read {}", extract_dir.display()))?
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();

    // the whole old tree, so a dir in both trees doesn't keep files the
    // new one dropped
    for entry in std::fs::read_dir(source_dir)
        .with_context(|| format!("failed to read {}", source_dir.display()))?
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if is_kept(&name) || keep.iter().any(|kept| kept.starts_with(&path)) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case("build", &["build"], true, true, Some("build-debmagic"); "own output dir moves aside")]
    #[test_case("build", &["build"], false, true, None; "untagged output dir")]
    #[test_case("build", &["build"], true, false, None; "clash configured to fail")]
    #[test_case("build", &["build", "build-debmagic"], true, true, None; "fallback shipped too")]
    #[test_case("build/out", &["build"], true, true, None; "output dir below a shipped dir")]
    #[test_case("build", &["src"], true, true, Some("build"); "output dir not shipped")]
    #[test_case("/elsewhere/out", &["build"], false, false, Some("/elsewhere/out"); "output dir outside the tree")]
    #[test_case("..", &["build"], false, false, Some(".."); "output dir is the parent")]
    fn test_make_room_for_upstream(
        output_dir: &str,
        shipped: &[&str],
        tagged: bool,
        fallback: bool,
        expected: Option<&str>,
    ) {
        let base = std::env::temp_dir().join(format!("debmagic-clash-{}", uuid::Uuid::new_v4()));
        let target_dir = base.join("pkg");
        let extract_dir = base.join("tree");
        let output_dir = target_dir.join(output_dir);
        std::fs::create_dir_all(&target_dir).unwrap();
        if tagged {
            crate::output_dir::create(&output_dir).unwrap();
        }
        for name in shipped {
            std::fs::create_dir_all(extract_dir.join(name)).unwrap();
        }
        let orig_tarball = crate::upstream::orig::OrigTarballConfig::default();
        let sign = crate::config::SignConfig::default();
        let fallback_dir = target_dir.join("build-debmagic");
        let options = FetchOptions {
            from: crate::upstream::fetch::FetchFrom::Upstream,
            orig_tarball: &orig_tarball,
            repack: false,
            signature_check: false,
            sign: &sign,
            output_dir: &output_dir,
            output_dir_fallback: fallback.then_some(fallback_dir.as_path()),
            force: false,
            dry_run: false,
        };

        let result = make_room_for_upstream(&target_dir, &extract_dir, &options);

        match expected {
            Some(expected) => {
                let kept = result.unwrap();
                assert_eq!(kept, target_dir.join(expected));
                if tagged {
                    assert!(crate::output_dir::is_tagged(&kept));
                }
            }
            None => assert!(result.is_err()),
        }
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn test_extract_and_swap() {
        let base = std::env::temp_dir().join(format!("debmagic-switch-{}", std::process::id()));
        let source_dir = base.join("pkg");
        let extract_dir = base.join("tree");
        std::fs::create_dir_all(source_dir.join("debian")).unwrap();
        std::fs::create_dir_all(&extract_dir).unwrap();
        // old tree: files that the new tree lacks, plus debian/
        std::fs::write(source_dir.join("old.txt"), "old").unwrap();
        std::fs::create_dir_all(source_dir.join("src")).unwrap();
        std::fs::write(source_dir.join("src").join("gone.c"), "x").unwrap();
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
            true,
        )
        .unwrap();

        assert!(source_dir.join("new.txt").exists());
        assert!(source_dir.join("src").join("main.c").exists());
        assert!(!source_dir.join("old.txt").exists());
        assert!(!source_dir.join("src").join("gone.c").exists());
        assert!(source_dir.join("debian").join("changelog").exists());
        assert!(output_dir.join("pkg_2.0.orig.tar.xz").exists());
        assert!(component_dir.join("plugin.c").exists());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn test_swap_component_replaces_debian() {
        let base =
            std::env::temp_dir().join(format!("debmagic-switch-comp-{}", std::process::id()));
        let component_dir = base.join("plugin");
        let extract_dir = base.join("tree");
        std::fs::create_dir_all(component_dir.join("debian")).unwrap();
        std::fs::create_dir_all(extract_dir.join("debian")).unwrap();
        std::fs::write(component_dir.join("debian").join("old"), "old").unwrap();
        std::fs::write(extract_dir.join("debian").join("new"), "new").unwrap();

        swap_tree(&component_dir, &extract_dir, &[], false).unwrap();

        assert!(component_dir.join("debian").join("new").exists());
        assert!(!component_dir.join("debian").join("old").exists());
        std::fs::remove_dir_all(&base).unwrap();
    }
}
