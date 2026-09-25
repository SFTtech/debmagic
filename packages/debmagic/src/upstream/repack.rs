use std::path::Path;

use anyhow::Context;

/// The repack configuration for one source: which files to strip from
/// the upstream tree and which version suffix the repack warrants.
#[derive(Debug, Default, Clone)]
pub struct RepackConfig {
    /// Glob patterns to exclude, from `Files-Excluded` in
    /// `debian/copyright` and the watch file's `Repack`/`Repacksuffix`.
    pub excludes: Vec<String>,
    /// Suffix appended to the upstream version, e.g. `+dfsg1`.
    pub suffix: Option<String>,
}

/// Load the repack configuration from the package's `debian/copyright`
/// (`Files-Excluded`, and `Files-Excluded-<component>` for MUT
/// packages) merged with the watch source's repack options.
pub fn load_repack_config(
    package: &debmagic_common::package::SourcePackage,
    watch: &crate::upstream::watch::WatchSource,
) -> anyhow::Result<RepackConfig> {
    let excluded = package.copyright_excludes()?;
    let mut excludes = excluded.main;
    match &watch.component {
        // a MUT source only excludes its own component's patterns
        Some(component) => excludes.extend(
            excluded
                .components
                .get(component)
                .cloned()
                .unwrap_or_default(),
        ),
        // a non-component source applies every field, as uscan does
        None => excludes.extend(excluded.components.into_values().flatten()),
    }

    if watch.repack && excludes.is_empty() {
        anyhow::bail!(
            "the watch file requests a repack but debian/copyright has no Files-Excluded"
        );
    }
    Ok(RepackConfig {
        excludes,
        suffix: watch.repack_suffix.clone(),
    })
}

/// Whether any exclude pattern matches `path` (relative, forward slashes).
/// A `dir/*` pattern excludes everything under `dir`, at any depth.
fn is_excluded(path: &str, excludes: &[String]) -> bool {
    excludes.iter().any(|pattern| {
        let Ok(glob) = glob::Pattern::new(pattern) else {
            return false;
        };
        if glob.matches(path) {
            return true;
        }
        // the pattern may match an ancestor: `win32/*` covers win32/nested/b.c
        let mut ancestor = String::new();
        path.split('/').any(|part| {
            ancestor.push_str(part);
            ancestor.push('/');
            glob.matches(&ancestor)
        })
    })
}

/// Remove all files matching the exclude patterns from `tree_dir`.
/// Returns the number of removed files.
pub fn apply_excludes(tree_dir: &Path, excludes: &[String]) -> anyhow::Result<usize> {
    let mut removed = 0;
    let mut stack = vec![tree_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in
            std::fs::read_dir(&dir).with_context(|| format!("failed to read {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            let relative = path
                .strip_prefix(tree_dir)
                .expect("walked path is under the tree")
                .to_string_lossy()
                .into_owned();
            if is_excluded(&relative, excludes) {
                if path.is_dir() {
                    std::fs::remove_dir_all(&path).with_context(|| {
                        format!("failed to remove excluded dir {}", path.display())
                    })?;
                } else {
                    std::fs::remove_file(&path).with_context(|| {
                        format!("failed to remove excluded file {}", path.display())
                    })?;
                }
                removed += 1;
            } else if path.is_dir() {
                stack.push(path);
            }
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn watch() -> crate::upstream::watch::WatchSource {
        crate::upstream::watch::WatchSource::default()
    }

    fn package_with_copyright(copyright: &str) -> debmagic_common::package::SourcePackage {
        debmagic_common::package::SourcePackage::from_files([
            (
                "changelog",
                "pkg (1.0-1) unstable; urgency=medium\n\n  * Change.\n\n -- A <a@example.com>  Mon, 01 Jan 2024 00:00:00 +0000\n",
            ),
            ("copyright", copyright),
        ])
        .unwrap()
    }

    #[test]
    fn test_load_repack_config_from_copyright() {
        let package = package_with_copyright(
            "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\nFiles-Excluded:\n win32/*\n docs/html/*\n\nFiles: *\nCopyright: X\nLicense: MIT\n",
        );

        let config = load_repack_config(&package, &watch()).unwrap();
        assert_eq!(config.excludes, vec!["win32/*", "docs/html/*"]);
        assert_eq!(config.suffix, None);
    }

    #[test]
    fn test_load_repack_config_component_field() {
        let package = package_with_copyright(
            "Files-Excluded-pigeonhole: doc/rfc\n\nFiles: *\nCopyright: X\n",
        );

        let config = load_repack_config(&package, &watch()).unwrap();
        assert_eq!(config.excludes, vec!["doc/rfc"]);
    }

    #[test]
    fn test_load_repack_config_suffix_from_watch() {
        let package = package_with_copyright("Files: *\nCopyright: X\n");

        let mut source = watch();
        source.repack_suffix = Some("+dfsg1".to_string());
        let config = load_repack_config(&package, &source).unwrap();
        assert_eq!(config.suffix.as_deref(), Some("+dfsg1"));
    }

    #[test]
    fn test_apply_excludes() {
        let dir = std::env::temp_dir().join(format!("debmagic-excl-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("win32").join("nested")).unwrap();
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("win32").join("a.c"), "x").unwrap();
        std::fs::write(dir.join("win32").join("nested").join("b.c"), "x").unwrap();
        std::fs::write(dir.join("src").join("main.c"), "x").unwrap();
        std::fs::write(dir.join("top.c"), "x").unwrap();

        let removed = apply_excludes(&dir, &["win32/*".to_string()]).unwrap();
        assert_eq!(removed, 1);
        assert!(!dir.join("win32").exists());
        assert!(dir.join("src").join("main.c").exists());
        assert!(dir.join("top.c").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_is_excluded() {
        let excludes = vec!["win32/*".to_string(), "*.pdf".to_string()];
        assert!(is_excluded("win32/a.c", &excludes));
        assert!(is_excluded("win32/nested/b.c", &excludes));
        assert!(is_excluded("docs/manual.pdf", &excludes));
        assert!(!is_excluded("src/main.c", &excludes));
        assert!(!is_excluded("win32.c", &excludes));
    }
}
