use std::str::FromStr;

use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageVersion {
    // distro packaging override base version (default is 0)
    epoch: String,

    // upstream package version
    upstream: String,

    // packaging (linux distro) revision
    revision: String,
}

impl PackageVersion {
    pub fn version(&self) -> String {
        let mut ret: String = "".to_owned();
        if self.epoch != "0" {
            ret.push_str(&format!("{}:", self.epoch));
        }
        ret.push_str(&self.upstream);
        if !self.revision.is_empty() {
            ret.push_str(&format!("-{}", self.revision));
        }
        ret
    }

    /// distro epoch plus upstream version
    pub fn epoch_upstream(&self) -> String {
        if !self.epoch.is_empty() {
            return format!("{}:{}", self.epoch, self.upstream);
        }
        self.upstream.clone()
    }

    /// distro epoch plus upstream version
    pub fn upstream_revision(&self) -> String {
        if !self.revision.is_empty() {
            return format!("{}-{}", self.upstream, self.revision);
        }
        self.upstream.clone()
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
            "0".to_string()
        } else {
            let re_epoch = Regex::new(r"^([0-9]+):.*$").map_err(|_| VersionParseError)?;
            re_epoch.replace(version, "$1").to_string()
        };

        let re_upstream = Regex::new(r"^([0-9]*:)?(.*?)$").map_err(|_| VersionParseError)?;
        let upstream = re_upstream.replace(&epoch_upstream, "$2").to_string();

        let re_revision = Regex::new(r"^.*?(-([^-]*))?$").map_err(|_| VersionParseError)?;
        let revision = re_revision.replace(version, "$2").to_string();

        // TODO: properly handle errors if we put in actual crap -> currently we return something nonsensical instead of returning an error

        Ok(Self {
            epoch,
            upstream,
            revision,
        })
    }
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;

    #[test_case(
        "1.2.3a.4-42.2-14ubuntu2~20.04.1", 
        &PackageVersion{epoch:"0".to_string(), upstream:"1.2.3a.4-42.2".to_string(), revision:"14ubuntu2~20.04.1".to_string()})]
    #[test_case(
        "3:1.2.3a.4-42.2-14ubuntu2~20.04.1", 
        &PackageVersion{epoch:"3".to_string(), upstream:"1.2.3a.4-42.2".to_string(), revision:"14ubuntu2~20.04.1".to_string()})]
    #[test_case(
        "3:1.2.3a.4ubuntu", 
        &PackageVersion{epoch:"3".to_string(), upstream:"1.2.3a.4ubuntu".to_string(), revision:"".to_string()})]
    #[test_case(
        "3:1.2.3a-4ubuntu", 
        &PackageVersion{epoch:"3".to_string(), upstream:"1.2.3a".to_string(), revision:"4ubuntu".to_string()})]
    #[test_case(
        "3:1.2.3a-4ubuntu1", 
        &PackageVersion{epoch:"3".to_string(), upstream:"1.2.3a".to_string(), revision:"4ubuntu1".to_string()})]
    fn test_version_parsing(version: &str, expected: &PackageVersion) {
        let parsed_version = PackageVersion::from_str(version).unwrap();

        // initial parsing works
        assert_eq!(&parsed_version, expected);
        // reverse formatting works as well
        assert_eq!(parsed_version.version(), version);
    }
}
