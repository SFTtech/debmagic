//! The ar members of a Binary package, in archive order.
//!
//! This is the `.deb` container (deb(5)): `debian-binary`, the Control
//! tarball, the Data tarball, and whatever else the archive carries. It is
//! what lintian's `deb-format` Check reads through `ar t`. The tarballs'
//! contents are Control files and Data files, not this.

use crate::lint::data;

/// One ar member: its name as `ar t` prints it and its size in bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DebMember {
    name: String,
    #[allow(dead_code)]
    size: u64,
}

impl DebMember {
    pub(crate) fn new(name: impl Into<String>, size: u64) -> Self {
        Self {
            name: name.into(),
            size,
        }
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Size of the member's content in bytes.
    #[allow(dead_code)]
    pub(crate) fn size(&self) -> u64 {
        self.size
    }
}

/// Every ar member of a Binary package, in archive order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct DebMembers {
    members: Vec<DebMember>,
}

/// Where lintian's `deb-format` Check places the Control and Data tarballs:
/// the first two members after `debian-binary` whose name does not start with
/// `_`. `None` when there is no such member.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TarballPositions {
    pub(crate) control: Option<usize>,
    pub(crate) data: Option<usize>,
}

impl DebMembers {
    pub(crate) fn new(members: Vec<DebMember>) -> Self {
        Self { members }
    }

    /// The canonical `debian-binary`, `control.tar.xz`, `data.tar.xz`.
    #[cfg(test)]
    pub(crate) fn canonical() -> Self {
        Self::from_names(["debian-binary", "control.tar.xz", "data.tar.xz"])
    }

    #[cfg(test)]
    pub(crate) fn from_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Self {
        Self::new(
            names
                .into_iter()
                .map(|name| DebMember::new(name, 0))
                .collect(),
        )
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &DebMember> {
        self.members.iter()
    }

    /// The archive has the shape `deb-format` can inspect: at least three
    /// members, the first of them `debian-binary`.
    pub(crate) fn is_well_shaped(&self) -> bool {
        self.members.len() >= 3 && self.members[0].name == "debian-binary"
    }

    /// Positions of the Control and Data tarballs. Members whose name starts
    /// with `_` are skipped, as dpkg-deb does.
    pub(crate) fn tarball_positions(&self) -> TarballPositions {
        let control = (1..self.members.len()).find(|&i| !self.members[i].name.starts_with('_'));
        let data = control.and_then(|control| {
            (control + 1..self.members.len()).find(|&i| !self.members[i].name.starts_with('_'))
        });
        TarballPositions { control, data }
    }
}

/// Where a member that is neither `debian-binary` nor a tarball may appear:
/// lintian's `deb-format/extra-members`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExtraMemberPosition {
    Anywhere,
    At(usize),
}

/// Position the vendored `extra-members` table expects `name` at, if the
/// table lists it.
pub(crate) fn expected_extra_member_position(name: &str) -> Option<ExtraMemberPosition> {
    data::lines(include_str!("data/deb-format/extra-members")).find_map(|line| {
        let (member, position) = line.split_once("=>")?;
        if member.trim() != name {
            return None;
        }
        let position = position.trim();
        if position == "ANYWHERE" {
            Some(ExtraMemberPosition::Anywhere)
        } else {
            position.parse().ok().map(ExtraMemberPosition::At)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_members_are_the_tarballs() {
        let members = DebMembers::canonical();
        assert!(members.is_well_shaped());
        assert_eq!(
            members.tarball_positions(),
            TarballPositions {
                control: Some(1),
                data: Some(2)
            }
        );
    }

    #[test]
    fn underscore_members_are_skipped_when_finding_tarballs() {
        let members = DebMembers::from_names([
            "debian-binary",
            "_before",
            "control.tar.xz",
            "_between",
            "data.tar.xz",
        ]);
        assert_eq!(
            members.tarball_positions(),
            TarballPositions {
                control: Some(2),
                data: Some(4)
            }
        );
    }

    #[test]
    fn only_underscore_members_after_debian_binary_leave_no_tarballs() {
        let members = DebMembers::from_names(["debian-binary", "_a", "_b"]);
        assert_eq!(
            members.tarball_positions(),
            TarballPositions {
                control: None,
                data: None
            }
        );
    }

    #[test]
    fn shape_needs_three_members_and_a_leading_debian_binary() {
        assert!(!DebMembers::from_names(["debian-binary", "control.tar"]).is_well_shaped());
        assert!(
            !DebMembers::from_names(["control.tar.xz", "debian-binary", "data.tar.xz"])
                .is_well_shaped()
        );
    }

    #[test]
    fn debian_lists_no_extra_members() {
        assert_eq!(expected_extra_member_position("anything"), None);
    }
}
