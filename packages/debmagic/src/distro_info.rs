//! The Debian and Ubuntu release table from distro-info-data: the system's
//! copy if it's current, else one fetched from upstream into the cache.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::Context;
use debmagic_common::distro::DistroTable;

const DATA_DIR: &str = "usr/share/distro-info";
const UPSTREAM: &str = "https://salsa.debian.org/debian/distro-info-data/-/raw/main";
const DEBIAN_CSV: &str = "debian.csv";
const UBUNTU_CSV: &str = "ubuntu.csv";

/// Picks up upstream corrections and announced release dates, which the
/// outdated check can't see.
const CACHE_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);
/// Upstream lags behind a release for a while: don't refetch it on every run.
const OUTDATED_RETRY_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// The release table as of today, from the first current copy of
/// distro-info-data: the system's, the cached one, a freshly fetched one.
/// When fetching fails, an outdated copy is used with a warning.
pub async fn load() -> anyhow::Result<DistroTable> {
    let system_dirs = system_dirs(std::env::var_os("SNAP").map(PathBuf::from).as_deref());
    let mut host = None;
    for dir in &system_dirs {
        match read(dir).transpose()? {
            Some(table) if table.outdated().is_empty() => return Ok(table),
            table => host = host.or(table),
        }
    }

    let cache_dir = dirs::cache_dir()
        .map(|cache| cache.join("debmagic").join("distro-info"))
        .context("no cache dir to keep distro-info-data in")?;
    let mut cached = read(&cache_dir).transpose()?;
    let age = cache_age(&cache_dir);
    if let Some(table) = cached.take_if(|table| {
        let max_age = match table.outdated() {
            [] => CACHE_MAX_AGE,
            _ => OUTDATED_RETRY_AGE,
        };
        age.is_some_and(|age| age < max_age)
    }) {
        warn_if_outdated(&table, &cache_dir.display().to_string());
        return Ok(table);
    }

    match (fetch(&cache_dir).await, cached.or(host)) {
        (Ok(table), _) => {
            warn_if_outdated(&table, UPSTREAM);
            Ok(table)
        }
        (Err(error), Some(stale)) => {
            eprintln!(
                "debmagic: warning: using outdated distro-info-data, updating it failed: {error:#}"
            );
            Ok(stale)
        }
        (Err(error), None) => {
            let searched: Vec<String> = system_dirs
                .iter()
                .chain([&cache_dir])
                .map(|dir| dir.display().to_string())
                .collect();
            Err(error.context(format!(
                "no distro-info-data in {}; install distro-info-data",
                searched.join(", ")
            )))
        }
    }
}

/// Where distro-info-data may be installed: the system, and in a snap
/// (whose `/usr` is its base's under strict confinement) the snap itself and
/// its base snap.
fn system_dirs(snap: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = vec![Path::new("/").join(DATA_DIR)];
    if let Some(snap) = snap {
        dirs.push(snap.join(DATA_DIR));
        let meta = std::fs::read_to_string(snap.join("meta/snap.yaml")).unwrap_or_default();
        dirs.extend(
            snap_base(&meta)
                .map(|base| Path::new("/snap").join(base).join("current").join(DATA_DIR)),
        );
    }
    dirs
}

/// The `base` of a snap from its `meta/snap.yaml`.
fn snap_base(meta: &str) -> Option<&str> {
    meta.lines()
        .find_map(|line| line.strip_prefix("base:"))
        .map(|base| base.trim().trim_matches(['"', '\'']))
        .filter(|base| !base.is_empty())
}

fn today() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

/// The table from the CSVs in `dir`, `None` if it has none.
fn read(dir: &Path) -> Option<anyhow::Result<DistroTable>> {
    let debian = std::fs::read_to_string(dir.join(DEBIAN_CSV)).ok()?;
    let table = std::fs::read_to_string(dir.join(UBUNTU_CSV))
        .with_context(|| format!("reading {UBUNTU_CSV} failed"))
        .and_then(|ubuntu| DistroTable::from_distro_info(&debian, &ubuntu, today()))
        .with_context(|| format!("loading the distro-info-data in {} failed", dir.display()));
    Some(table)
}

fn cache_age(dir: &Path) -> Option<Duration> {
    let modified = std::fs::metadata(dir.join(DEBIAN_CSV))
        .ok()?
        .modified()
        .ok()?;
    SystemTime::now().duration_since(modified).ok()
}

/// Fetch the CSVs from upstream into `dir`, only once both parse.
async fn fetch(dir: &Path) -> anyhow::Result<DistroTable> {
    let debian = crate::requests::http_get(&format!("{UPSTREAM}/{DEBIAN_CSV}")).await?;
    let ubuntu = crate::requests::http_get(&format!("{UPSTREAM}/{UBUNTU_CSV}")).await?;
    let table = DistroTable::from_distro_info(&debian, &ubuntu, today())
        .context("loading the fetched distro-info-data failed")?;
    std::fs::create_dir_all(dir).with_context(|| format!("creating {} failed", dir.display()))?;
    for (name, content) in [(UBUNTU_CSV, &ubuntu), (DEBIAN_CSV, &debian)] {
        write_atomic(&dir.join(name), content)?;
    }
    Ok(table)
}

fn write_atomic(path: &Path, content: &str) -> anyhow::Result<()> {
    let partial = PathBuf::from(format!("{}.part", path.display()));
    std::fs::write(&partial, content)
        .and_then(|()| std::fs::rename(&partial, path))
        .with_context(|| format!("writing {} failed", path.display()))
}

fn warn_if_outdated(table: &DistroTable, source: &str) {
    let outdated: Vec<&str> = table
        .outdated()
        .iter()
        .map(|distro| distro.as_str())
        .collect();
    if !outdated.is_empty() {
        eprintln!(
            "debmagic: warning: the distro-info-data from {source} predates the latest {} release",
            outdated.join(" and ")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_also_searches_itself_and_its_base() {
        let dir = std::env::temp_dir().join(format!("debmagic-snap-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("meta")).unwrap();
        std::fs::write(dir.join("meta/snap.yaml"), "name: debmagic\nbase: core26\n").unwrap();
        assert_eq!(
            system_dirs(Some(&dir)),
            [
                PathBuf::from("/usr/share/distro-info"),
                dir.join("usr/share/distro-info"),
                PathBuf::from("/snap/core26/current/usr/share/distro-info"),
            ]
        );
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(system_dirs(None), [PathBuf::from("/usr/share/distro-info")]);
    }
}
