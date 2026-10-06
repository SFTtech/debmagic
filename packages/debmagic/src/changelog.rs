use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};

use debmagic_common::debian::changelog::Maintainer;
use debmagic_common::debian::source::SourceFormat;

/// Inputs for the `debmagic changelog new` command.
#[derive(Debug, Clone)]
pub struct ChangelogNewCommand {
    pub source_dir: PathBuf,
}

/// Run `debmagic changelog new`: prepend an `UNRELEASED` entry that keeps
/// the current version, signed by the user.
pub fn new_entry(command: ChangelogNewCommand) -> anyhow::Result<()> {
    let source_dir = command.source_dir;
    let path = source_dir.join("debian").join("changelog");
    let mut changelog = load_changelog(&source_dir)?;
    let head = changelog_head(&changelog)
        .with_context(|| format!("failed to read the head entry of {}", path.display()))?;
    let maintainer = maintainer(&source_dir, |name| std::env::var(name).ok())?;
    debmagic_common::debian::changelog::add_entry(
        &mut changelog,
        &head.version,
        maintainer,
        chrono::Local::now().fixed_offset(),
    )
    .with_context(|| format!("adding an entry to {} failed", path.display()))?;

    let partial = path.with_extension("dch");
    std::fs::write(&partial, changelog.to_string())
        .with_context(|| format!("failed to write {}", partial.display()))?;
    std::fs::rename(&partial, &path)
        .with_context(|| format!("failed to replace {}", path.display()))?;
    println!(
        "debmagic: added {} ({}) UNRELEASED to {}",
        head.package,
        head.version,
        path.display()
    );
    Ok(())
}

/// The user signing changelog entries, found like dch does (`DEBFULLNAME`,
/// `DEBEMAIL`, `NAME`, `EMAIL`, where an email var may be `Name <email>`),
/// then from git config. Never guessed from the login or hostname.
fn maintainer(
    source_dir: &Path,
    get_env: impl Fn(&str) -> Option<String>,
) -> anyhow::Result<Maintainer> {
    let env = |key| get_env(key).filter(|value: &String| !value.trim().is_empty());
    let split = |value: String| -> (Option<String>, String) {
        let (name, email) = debian_changelog::parseaddr(&value);
        (name.map(str::to_string), email.to_string())
    };
    let (debemail_name, debemail) = env("DEBEMAIL").map(split).unzip();
    let (email_name, email) = env("EMAIL").map(split).unzip();
    let name = env("DEBFULLNAME")
        .or(debemail_name.flatten())
        .or_else(|| env("NAME"))
        .or(email_name.flatten())
        .or_else(|| git_config(source_dir, "user.name"));
    let email = debemail
        .or(email)
        .or_else(|| git_config(source_dir, "user.email"));
    match (name, email) {
        (Some(name), Some(email)) => Ok((name.trim().to_string(), email.trim().to_string())),
        _ => bail!(
            "unknown changelog maintainer: set DEBFULLNAME and DEBEMAIL (or git's user.name and user.email)"
        ),
    }
}

fn git_config(dir: &Path, key: &str) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["config", "--get", key])
        .output()
        .ok()?;
    let value = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (output.status.success() && !value.is_empty()).then_some(value)
}

/// Read `debian/source/format`; a missing file is `1.0`, like dpkg-source.
pub fn load_source_format(dir: &Path) -> SourceFormat {
    std::fs::read_to_string(dir.join("debian").join("source").join("format"))
        .map(|content| SourceFormat::parse(&content))
        .unwrap_or_default()
}

/// Read and parse `debian/changelog`.
pub fn load_changelog(dir: &Path) -> anyhow::Result<debian_changelog::ChangeLog> {
    let path = dir.join("debian").join("changelog");
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    content
        .parse()
        .with_context(|| format!("failed to parse {}", path.display()))
}

/// The head entry of `debian/changelog`, as the package identity.
pub fn load_changelog_head(
    dir: &Path,
) -> anyhow::Result<debmagic_common::debian::changelog::ChangelogHead> {
    let changelog = load_changelog(dir)?;
    changelog_head(&changelog).with_context(|| {
        format!(
            "failed to read the head entry of {}",
            dir.join("debian").join("changelog").display()
        )
    })
}

/// Extract the head entry of a parsed changelog.
pub fn changelog_head(
    changelog: &debian_changelog::ChangeLog,
) -> anyhow::Result<debmagic_common::debian::changelog::ChangelogHead> {
    use anyhow::bail;

    if changelog.iter().next().is_none() {
        bail!("changelog is empty");
    }
    debmagic_common::debian::changelog::ChangelogHead::from_changelog(changelog).ok_or_else(|| {
        anyhow::anyhow!("changelog head entry has no package, version or distribution")
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn maintainer_from(vars: &[(&str, &str)]) -> Maintainer {
        let vars: HashMap<_, _> = vars.iter().copied().collect();
        // a dir outside any git worktree with no config, so only the env counts
        maintainer(Path::new("/nonexistent"), |key| {
            vars.get(key).map(|value| value.to_string())
        })
        .unwrap()
    }

    #[test]
    fn test_maintainer_deb_vars() {
        assert_eq!(
            maintainer_from(&[
                ("DEBFULLNAME", "Jane Doe"),
                ("DEBEMAIL", "jane@example.com"),
                ("EMAIL", "other@example.com"),
            ]),
            ("Jane Doe".into(), "jane@example.com".into())
        );
    }

    #[test]
    fn test_maintainer_name_from_email_var() {
        assert_eq!(
            maintainer_from(&[("DEBEMAIL", "Jane Doe <jane@example.com>")]),
            ("Jane Doe".into(), "jane@example.com".into())
        );
    }

    #[test]
    fn test_maintainer_debfullname_wins_over_debemail_name() {
        assert_eq!(
            maintainer_from(&[
                ("DEBFULLNAME", "Jane Doe"),
                ("DEBEMAIL", "J. D. <jane@example.com>"),
            ]),
            ("Jane Doe".into(), "jane@example.com".into())
        );
    }

    #[test]
    fn test_maintainer_generic_vars() {
        assert_eq!(
            maintainer_from(&[("NAME", "Jane Doe"), ("EMAIL", "jane@example.com")]),
            ("Jane Doe".into(), "jane@example.com".into())
        );
    }
}
