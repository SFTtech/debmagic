use std::fmt;
use std::str::FromStr;

use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageVersion {
    // distro packaging override base version (default is 0)
    epoch: Option<u32>,

    // upstream package version
    upstream: String,

    // packaging (linux distro) revision
    revision: Option<String>,
}

impl PackageVersion {
    pub fn new(epoch: Option<u32>, upstream: String, revision: Option<String>) -> Self {
        Self {
            epoch,
            upstream,
            revision,
        }
    }

    pub fn version(&self) -> String {
        let mut ret: String = "".to_owned();
        if let Some(epoch) = self.epoch
            && epoch != 0
        {
            ret.push_str(&format!("{}:", epoch));
        }
        ret.push_str(&self.upstream);
        if let Some(revision) = &self.revision {
            ret.push_str(&format!("-{}", revision));
        }
        ret
    }

    /// distro epoch plus upstream version
    pub fn epoch_upstream(&self) -> String {
        if let Some(epoch) = self.epoch {
            return format!("{}:{}", epoch, self.upstream);
        }
        self.upstream.clone()
    }

    /// distro epoch plus upstream version
    pub fn upstream_revision(&self) -> String {
        if let Some(revision) = &self.revision {
            return format!("{}-{}", self.upstream, revision);
        }
        self.upstream.clone()
    }
}

impl fmt::Display for PackageVersion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.version())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct VersionParseError;

impl FromStr for PackageVersion {
    type Err = VersionParseError;

    fn from_str(version: &str) -> Result<Self, Self::Err> {
        let re_epoch_upstream = Regex::new(r"^(.*?)(-[^-]*)?$").map_err(|_| VersionParseError)?;
        let epoch_upstream = re_epoch_upstream.replace(version, "$1").to_string();

        // epoch = distro packaging override base version (default is 0)
        // pkg-info.mk uses the full version if no epoch is in it.
        // instead, we return "0" as oritinally intended if no epoch is in version.
        let epoch = if !version.contains(':') {
            Some(0)
        } else {
            let re_epoch = Regex::new(r"^([0-9]+):.*$").map_err(|_| VersionParseError)?;
            re_epoch.replace(version, "$1").to_string().parse().ok()
        };

        let re_upstream = Regex::new(r"^([0-9]*:)?(.*?)$").map_err(|_| VersionParseError)?;
        let upstream = re_upstream.replace(&epoch_upstream, "$2").to_string();

        let re_revision = Regex::new(r"^.*?(-([^-]*))?$").map_err(|_| VersionParseError)?;
        let revision = re_revision.replace(version, "$2").to_string();

        // TODO: properly handle errors if we put in actual crap -> currently we return something nonsensical instead of returning an error

        Ok(Self {
            epoch,
            upstream,
            revision: if revision.is_empty() {
                None
            } else {
                Some(revision)
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;

    #[test_case(
        "1.2.3a.4-42.2-14ubuntu2~20.04.1", 
        &PackageVersion{epoch:Some(0), upstream:"1.2.3a.4-42.2".to_string(), revision:Some("14ubuntu2~20.04.1".to_string())})]
    #[test_case(
        "3:1.2.3a.4-42.2-14ubuntu2~20.04.1", 
        &PackageVersion{epoch:Some(3), upstream:"1.2.3a.4-42.2".to_string(), revision:Some("14ubuntu2~20.04.1".to_string())})]
    #[test_case(
        "3:1.2.3a.4ubuntu", 
        &PackageVersion{epoch:Some(3), upstream:"1.2.3a.4ubuntu".to_string(), revision:None})]
    #[test_case(
        "3:1.2.3a-4ubuntu", 
        &PackageVersion{epoch:Some(3), upstream:"1.2.3a".to_string(), revision:Some("4ubuntu".to_string())})]
    #[test_case(
        "3:1.2.3a-4ubuntu1", 
        &PackageVersion{epoch:Some(3), upstream:"1.2.3a".to_string(), revision:Some("4ubuntu1".to_string())})]
    fn test_version_parsing(version: &str, expected: &PackageVersion) {
        let parsed_version = PackageVersion::from_str(version).unwrap();

        // initial parsing works
        assert_eq!(&parsed_version, expected);
        // reverse formatting works as well
        assert_eq!(parsed_version.version(), version);
    }
}
