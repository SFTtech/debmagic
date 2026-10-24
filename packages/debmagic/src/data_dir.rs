use std::{ffi::OsStr, path::PathBuf};

use anyhow::{Context, bail};

/// Machine-local data directory: `$DEBMAGIC_DATA_DIR`, or
/// `$XDG_DATA_HOME/debmagic` (`~/.local/share/debmagic`).
///
/// A relative `DEBMAGIC_DATA_DIR` is resolved to an absolute path against the
/// current directory, so later use does not depend on a different working
/// directory.
pub fn data_dir() -> anyhow::Result<PathBuf> {
    let override_dir = std::env::var_os("DEBMAGIC_DATA_DIR");
    resolve_data_dir(override_dir.as_deref(), dirs::data_dir().as_deref())
}

pub fn registry_path() -> anyhow::Result<PathBuf> {
    Ok(data_dir()?.join("db.sqlite"))
}

pub fn default_environments_dir() -> PathBuf {
    environments_dir_for(data_dir())
}

fn resolve_data_dir(
    override_dir: Option<&OsStr>,
    xdg_data_home: Option<&std::path::Path>,
) -> anyhow::Result<PathBuf> {
    if let Some(path) = override_dir {
        if path.is_empty() {
            bail!("DEBMAGIC_DATA_DIR is set but empty");
        }
        return std::path::absolute(path).context("resolving DEBMAGIC_DATA_DIR failed");
    }

    xdg_data_home
        .map(|path| path.join("debmagic"))
        .context("cannot determine the user data directory")
}

fn environments_dir_for(data: anyhow::Result<PathBuf>) -> PathBuf {
    data.unwrap_or_else(|_| PathBuf::from("/tmp/debmagic"))
        .join("environments")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn debmagic_data_dir_overrides_xdg() {
        let dir = resolve_data_dir(Some(OsStr::new("/tmp/debmagic-data-test")), None).unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/debmagic-data-test"));
        assert_eq!(
            environments_dir_for(Ok(dir)),
            PathBuf::from("/tmp/debmagic-data-test/environments")
        );
    }

    #[test]
    fn relative_debmagic_data_dir_is_absolute() {
        let dir = resolve_data_dir(Some(OsStr::new("debmagic-data")), None).unwrap();
        assert!(dir.is_absolute());
        assert!(dir.ends_with("debmagic-data"));
    }

    #[test]
    fn empty_debmagic_data_dir_is_rejected() {
        let error = resolve_data_dir(Some(OsStr::new("")), None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("empty"));
    }

    #[test]
    fn xdg_data_home_is_used_without_override() {
        let dir = resolve_data_dir(None, Some(Path::new("/xdg"))).unwrap();
        assert_eq!(dir, PathBuf::from("/xdg/debmagic"));
    }

    #[test]
    fn missing_data_dir_falls_back_for_environments() {
        let environments = environments_dir_for(Err(anyhow::anyhow!("no home")));
        assert_eq!(environments, PathBuf::from("/tmp/debmagic/environments"));
    }
}
