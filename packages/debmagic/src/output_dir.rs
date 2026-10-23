//! The output dir, where builds export artifacts and `upstream` puts orig
//! tarballs. A `CACHEDIR.TAG` marks it as debmagic's: that tells it apart
//! from a `build-area/` dir upstream ships, and backup tools skip it.

use std::path::{Component, Path, PathBuf};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::config::Config;

const TAG_NAME: &str = "CACHEDIR.TAG";
/// The fixed start of a tag, see <https://bford.info/cachedir/>.
const TAG_SIGNATURE: &str = "Signature: 8a477f597d28d172789f06886806bc55";
const OUTPUT_DIR_HINT: &str =
    "debmagic output dir: build artifacts and orig tarballs, not part of the package source.";

/// What to do when the configured output dir holds files debmagic didn't
/// put there, e.g. a `build/` dir upstream ships.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputDirClash {
    /// Use `output_dir_fallback` instead.
    #[default]
    Fallback,
    /// Fail, asking for another output dir.
    Fail,
}

/// Configured paths are relative to the package source dir, not debmagic's cwd.
fn configured_path(source_dir: &Path, dir: &Path) -> anyhow::Result<PathBuf> {
    std::path::absolute(source_dir.join(dir)).context("resolving the output dir failed")
}

/// Resolve the output dir: `-o` relative to debmagic's cwd, else the
/// configured one relative to the source dir, or its fallback when upstream owns it.
pub fn resolve(source_dir: &Path, config: &Config, cli: Option<&Path>) -> anyhow::Result<PathBuf> {
    if let Some(dir) = cli {
        return std::path::absolute(dir).context("resolving the output dir failed");
    }
    let configured = configured_path(source_dir, &config.output_dir)?;
    // only a dir in the tree can be upstream's; `..` is never debmagic's alone
    if !is_in_tree(&config.output_dir) || !is_foreign(&configured)? {
        return Ok(configured);
    }
    if config.output_dir_clash == OutputDirClash::Fail {
        bail!(
            "the output dir {} holds files debmagic didn't put there, likely upstream's; \
             set `output_dir` or pass `-o` to use another directory",
            configured.display()
        );
    }
    let fallback = configured_path(source_dir, &config.output_dir_fallback)?;
    if is_foreign(&fallback)? {
        bail!(
            "both the output dir {} and its fallback {} hold files debmagic didn't put there; \
             set `output_dir` or pass `-o` to use another directory",
            configured.display(),
            fallback.display()
        );
    }
    println!(
        "debmagic: {} belongs to upstream, using {}",
        configured.display(),
        fallback.display()
    );
    Ok(fallback)
}

/// Where `upstream switch` moves the output dir when the new upstream
/// version starts shipping a directory of its name; `None` when the
/// output dir is explicit (`-o`) or clashes are configured to fail.
pub fn switch_fallback(
    source_dir: &Path,
    config: &Config,
    cli: Option<&Path>,
) -> anyhow::Result<Option<PathBuf>> {
    if cli.is_some()
        || config.output_dir_clash == OutputDirClash::Fail
        || !is_in_tree(&config.output_dir)
    {
        return Ok(None);
    }
    configured_path(source_dir, &config.output_dir_fallback).map(Some)
}

/// Create the output dir and tag it as debmagic's, unless it holds other
/// files (`-o ..`, upstream's `build/`).
pub fn create(dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("failed to create {}", dir.display()))?;
    if !is_foreign(dir)? {
        tag(dir, OUTPUT_DIR_HINT)
            .with_context(|| format!("failed to tag {} as debmagic's", dir.display()))?;
    }
    Ok(())
}

/// Write a `CACHEDIR.TAG` into `dir`, with `hint` telling humans what it is.
pub fn tag(dir: &Path, hint: &str) -> std::io::Result<()> {
    if is_tagged(dir) {
        return Ok(());
    }
    std::fs::write(
        dir.join(TAG_NAME),
        format!("{TAG_SIGNATURE}\n# {hint}\n# More info: https://bford.info/cachedir/\n"),
    )
}

pub fn is_tagged(dir: &Path) -> bool {
    std::fs::read(dir.join(TAG_NAME))
        .is_ok_and(|content| content.starts_with(TAG_SIGNATURE.as_bytes()))
}

fn is_in_tree(dir: &Path) -> bool {
    dir.components()
        .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

/// Whether `dir` holds files debmagic didn't put there: it is untagged and
/// holds more than artifacts (an output dir from before the tag holds only those).
fn is_foreign(dir: &Path) -> anyhow::Result<bool> {
    if is_tagged(dir) {
        return Ok(false);
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", dir.display()));
        }
    };
    for entry in entries {
        let entry = entry.with_context(|| format!("failed to read {}", dir.display()))?;
        let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
        let name = entry.file_name();
        if is_dir || !debmagic_common::artifact::is_artifact_filename(&name.to_string_lossy()) {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    fn test_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("debmagic-output-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn config(clash: OutputDirClash) -> Config {
        Config {
            output_dir_clash: clash,
            ..Config::default()
        }
    }

    #[test_case(&[], "build-area"; "no output dir")]
    #[test_case(&["pkg_1.0-1_amd64.changes", "pkg_1.0.orig.tar.xz"], "build-area"; "untagged artifacts")]
    #[test_case(&["config.guess"], "build-debmagic"; "upstream output dir")]
    fn test_resolve(files: &[&str], expected: &str) {
        let source_dir = test_dir();
        if !files.is_empty() {
            std::fs::create_dir_all(source_dir.join("build-area")).unwrap();
        }
        for file in files {
            std::fs::write(source_dir.join("build-area").join(file), "").unwrap();
        }

        let resolved = resolve(&source_dir, &config(OutputDirClash::Fallback), None).unwrap();

        assert_eq!(resolved, source_dir.join(expected));
        std::fs::remove_dir_all(&source_dir).unwrap();
    }

    #[test]
    fn test_resolve_tagged_upstream_content_is_ours() {
        let source_dir = test_dir();
        create(&source_dir.join("build-area")).unwrap();
        std::fs::write(source_dir.join("build-area/config.guess"), "").unwrap();

        let resolved = resolve(&source_dir, &config(OutputDirClash::Fallback), None).unwrap();

        assert_eq!(resolved, source_dir.join("build-area"));
        std::fs::remove_dir_all(&source_dir).unwrap();
    }

    #[test]
    fn test_resolve_clash_fails_when_configured() {
        let source_dir = test_dir();
        std::fs::create_dir_all(source_dir.join("build-area/aux")).unwrap();

        let error = resolve(&source_dir, &config(OutputDirClash::Fail), None).unwrap_err();

        assert!(error.to_string().contains("didn't put there"));
        std::fs::remove_dir_all(&source_dir).unwrap();
    }

    #[test]
    fn test_configured_paths_are_relative_to_the_source_dir() {
        let source_dir = test_dir();
        let absolute = test_dir().join("artifacts");
        let config = Config {
            output_dir: PathBuf::from("relative/artifacts"),
            output_dir_fallback: absolute.clone(),
            ..Config::default()
        };

        assert_eq!(
            resolve(&source_dir, &config, None).unwrap(),
            source_dir.join("relative/artifacts")
        );
        assert_eq!(
            switch_fallback(&source_dir, &config, None).unwrap(),
            Some(absolute)
        );
        std::fs::remove_dir_all(&source_dir).unwrap();
    }

    #[test]
    fn test_absolute_configured_path_is_unchanged() {
        let source_dir = test_dir();
        let output_dir = test_dir().join("artifacts");
        let config = Config {
            output_dir: output_dir.clone(),
            ..Config::default()
        };

        assert_eq!(resolve(&source_dir, &config, None).unwrap(), output_dir);
        std::fs::remove_dir_all(&source_dir).unwrap();
    }

    #[test]
    fn test_relative_cli_path_is_relative_to_the_cwd() {
        let source_dir = test_dir();
        let relative = PathBuf::from(format!("debmagic-cli-output-{}", uuid::Uuid::new_v4()));

        assert_eq!(
            resolve(
                &source_dir,
                &config(OutputDirClash::Fallback),
                Some(&relative)
            )
            .unwrap(),
            std::path::absolute(&relative).unwrap()
        );
        std::fs::remove_dir_all(&source_dir).unwrap();
    }

    #[test]
    fn test_absolute_cli_path_is_unchanged() {
        let source_dir = test_dir();
        let output_dir = test_dir().join("artifacts");

        assert_eq!(
            resolve(
                &source_dir,
                &config(OutputDirClash::Fallback),
                Some(&output_dir)
            )
            .unwrap(),
            output_dir
        );
        std::fs::remove_dir_all(&source_dir).unwrap();
    }

    #[test]
    fn test_resolve_outside_the_tree_is_taken_as_is() {
        let source_dir = test_dir();
        let config = Config {
            output_dir: PathBuf::from(".."),
            ..Config::default()
        };

        let resolved = resolve(&source_dir, &config, None).unwrap();

        assert_eq!(resolved, source_dir.join(".."));
        std::fs::remove_dir_all(&source_dir).unwrap();
    }

    #[test]
    fn test_create_tags_only_debmagic_dirs() {
        let base = test_dir();
        create(&base.join("new")).unwrap();
        std::fs::create_dir_all(base.join("upstream/aux")).unwrap();
        create(&base.join("upstream")).unwrap();

        assert!(is_tagged(&base.join("new")));
        assert!(!is_tagged(&base.join("upstream")));
        assert!(
            std::fs::read_to_string(base.join("new").join(TAG_NAME))
                .unwrap()
                .contains("debmagic")
        );
        std::fs::remove_dir_all(&base).unwrap();
    }
}
