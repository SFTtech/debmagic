use crate::debian::version::PackageVersion;

/// The head entry of a `debian/changelog`: what identifies the
/// package and where the upload is headed.
#[derive(Debug, Clone, PartialEq)]
pub struct ChangelogHead {
    pub package: String,
    pub version: PackageVersion,
    pub distributions: Vec<String>,
}

impl ChangelogHead {
    /// Extract the head entry of a parsed changelog.
    pub fn from_changelog(changelog: &debian_changelog::ChangeLog) -> Option<Self> {
        let first = changelog.iter().next()?;
        Some(Self {
            package: first.package()?,
            version: PackageVersion::new(
                first.version()?.epoch,
                first.version()?.upstream_version,
                first.version()?.debian_revision,
            ),
            distributions: first.distributions()?,
        })
    }
}

/// Who signs off a changelog entry, as `(name, email)`.
pub type Maintainer = (String, String);

/// Prepend an `UNRELEASED` entry for the head's package at `version`, with an
/// empty bullet to fill in. Identity and time come from the caller.
pub fn add_entry(
    changelog: &mut debian_changelog::ChangeLog,
    version: &PackageVersion,
    maintainer: Maintainer,
    timestamp: chrono::DateTime<chrono::FixedOffset>,
) -> anyhow::Result<()> {
    let head = changelog
        .iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("changelog has no entry to base the new one on"))?;
    if head.is_unreleased() == Some(true) {
        anyhow::bail!(
            "the top entry ({}) is already UNRELEASED; edit it instead",
            head.version()
                .map_or_else(|| "no version".into(), |v| v.to_string())
        );
    }
    let package = head
        .package()
        .ok_or_else(|| anyhow::anyhow!("the top entry has no package name"))?;
    let version = version
        .version()
        .parse()
        .map_err(|error| anyhow::anyhow!("invalid version {version}: {error}"))?;
    changelog
        .new_empty_entry()
        .package(package)
        .version(version)
        .distribution(debian_changelog::UNRELEASED.into())
        .urgency(debian_changelog::Urgency::Medium)
        .maintainer(maintainer)
        .datetime(timestamp)
        .change_line("* ".into())
        .finish();
    Ok(())
}

/// Whether a changelog's top entry looks like an Ubuntu deltarebase
/// onto a Debian version Ubuntu has never seen: the current revision
/// carries an `ubuntu` component while the previous entry's does not.
/// In that case the Ubuntu archive cannot have the orig tarball yet.
pub fn is_deltarebase_onto_debian(changelog: &debian_changelog::ChangeLog) -> bool {
    let mut entries = changelog.iter().take(2);
    let Some(current) = entries.next() else {
        return false;
    };
    let Some(previous) = entries.next() else {
        return false;
    };
    let Some(current_version) = current.version() else {
        return false;
    };
    let Some(previous_version) = previous.version() else {
        return false;
    };
    let current_is_ubuntu = revision_is_ubuntu(current_version.debian_revision.as_deref());
    let previous_is_ubuntu = revision_is_ubuntu(previous_version.debian_revision.as_deref());
    current_is_ubuntu && !previous_is_ubuntu
}

/// Whether a changelog's top entry bumps the upstream part relative
/// to the entry below: the orig tarball of the new upstream version
/// cannot be in the archive yet, so an upload must carry it.
pub fn is_new_upstream_version(changelog: &debian_changelog::ChangeLog) -> bool {
    let mut entries = changelog.iter().take(2);
    let Some(current) = entries.next() else {
        return false;
    };
    let Some(previous) = entries.next() else {
        // a new package, like dpkg-genchanges' default -si
        return true;
    };
    let (Some(current), Some(previous)) = (current.version(), previous.version()) else {
        return false;
    };
    current.upstream_version != previous.upstream_version
}

/// A Debian revision contains an `ubuntu` component, e.g. `1ubuntu2`.
fn revision_is_ubuntu(revision: Option<&str>) -> bool {
    revision.is_some_and(|r| r.contains("ubuntu"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(changelog: &str) -> debian_changelog::ChangeLog {
        changelog.parse().unwrap()
    }

    fn entry(version: &str) -> String {
        format!(
            "foo ({version}) unstable; urgency=medium\n\n  * Some change.\n\n -- A <a@example.com>  Mon, 01 Jan 2024 00:00:00 +0000\n"
        )
    }

    #[test]
    fn test_deltarebase_detected() {
        let changelog = parse(&format!(
            "{}\n{}",
            entry("1.2.3-1ubuntu1"),
            entry("1.2.3-1")
        ));
        assert!(is_deltarebase_onto_debian(&changelog));
    }

    #[test]
    fn test_consecutive_ubuntu_uploads_not_deltarebase() {
        let changelog = parse(&format!(
            "{}\n{}",
            entry("1.2.3-1ubuntu2"),
            entry("1.2.3-1ubuntu1")
        ));
        assert!(!is_deltarebase_onto_debian(&changelog));
    }

    #[test]
    fn test_pure_debian_not_deltarebase() {
        let changelog = parse(&format!("{}\n{}", entry("1.2.3-2"), entry("1.2.3-1")));
        assert!(!is_deltarebase_onto_debian(&changelog));
    }

    #[test]
    fn test_single_entry_not_deltarebase() {
        let changelog = parse(&entry("1.2.3-1ubuntu1"));
        assert!(!is_deltarebase_onto_debian(&changelog));
    }

    #[test]
    fn test_new_upstream_version_detected() {
        let changelog = parse(&format!(
            "{}\n{}",
            entry("1.2.3-1ubuntu1"),
            entry("1.2.2-3ubuntu4")
        ));
        assert!(is_new_upstream_version(&changelog));
    }

    #[test]
    fn test_same_upstream_not_new_upstream() {
        let changelog = parse(&format!(
            "{}\n{}",
            entry("1.2.3-1ubuntu2"),
            entry("1.2.3-1ubuntu1")
        ));
        assert!(!is_new_upstream_version(&changelog));
    }

    #[test]
    fn test_new_upstream_single_entry() {
        let changelog = parse(&entry("1.2.3-1"));
        assert!(is_new_upstream_version(&changelog));
    }

    #[test]
    fn test_head_from_changelog() {
        let changelog = parse(&entry("1.2.3-1ubuntu2"));
        let head = ChangelogHead::from_changelog(&changelog).unwrap();
        assert_eq!(head.package, "foo");
        assert_eq!(head.version.version(), "1.2.3-1ubuntu2");
        assert_eq!(head.distributions, ["unstable"]);
    }

    fn add(changelog: &str) -> anyhow::Result<String> {
        let mut changelog = parse(changelog);
        let version = ChangelogHead::from_changelog(&changelog).unwrap().version;
        let timestamp = chrono::DateTime::parse_from_rfc2822("Tue, 06 Oct 2026 12:34:56 +0200")?;
        add_entry(
            &mut changelog,
            &version,
            ("Jane Doe".into(), "jane@example.com".into()),
            timestamp,
        )?;
        Ok(changelog.to_string())
    }

    #[test]
    fn test_add_entry_copies_version() {
        let old = format!("{}\n{}", entry("1:1.2.3-1ubuntu2"), entry("1:1.2.3-1"));
        assert_eq!(
            add(&old).unwrap(),
            format!(
                "foo (1:1.2.3-1ubuntu2) UNRELEASED; urgency=medium\n\n  * \n\n -- Jane Doe <jane@example.com>  Tue, 06 Oct 2026 12:34:56 +0200\n\n{old}"
            )
        );
    }

    #[test]
    fn test_add_entry_result_parses() {
        let new = add(&entry("1.2.3-1")).unwrap();
        let head = ChangelogHead::from_changelog(&parse(&new)).unwrap();
        assert_eq!(head.version.version(), "1.2.3-1");
        assert_eq!(head.distributions, ["UNRELEASED"]);
        assert_eq!(parse(&new).iter().count(), 2);
    }

    #[test]
    fn test_add_entry_refuses_on_unreleased_head() {
        let old = entry("1.2.3-1").replace("unstable", "UNRELEASED");
        let error = add(&old).unwrap_err().to_string();
        assert!(error.contains("already UNRELEASED"), "{error}");
    }

    #[test]
    fn test_head_from_empty_changelog() {
        let changelog = parse("");
        assert!(ChangelogHead::from_changelog(&changelog).is_none());
    }
}
