use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::deb_members::{ExtraMemberPosition, expected_extra_member_position};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct MisplacedExtraMemberInDeb;

declare_rule! {
    /// The `.deb` has an ar member that is neither `debian-binary` nor the
    /// Control or Data tarball, and that lintian's `deb-format/extra-members`
    /// table does not allow at that position.
    ///
    /// The Control tarball is the first member after `debian-binary` whose
    /// name does not start with `_`, and the Data tarball the next one; every
    /// other member after `debian-binary` is an extra. Extra is
    /// `<member> (<text> at position <N>)`, with text `unexpected _member`
    /// for a name starting with `_`, `unexpected member` otherwise, and
    /// `expected at position <M>, but appeared` for a name the table lists at
    /// another position. Debian's table is empty, so every extra is reported.
    /// No Location; the Subject is the location.
    ///
    /// Reported for debs, udebs and ddebs. Skipped when the archive has fewer
    /// than three members or does not start with `debian-binary`; that is
    /// `malformed-deb-archive`, which a Binary package that cannot be opened
    /// never reaches. Member names are read lossily as UTF-8.
    MisplacedExtraMemberInDeb,
    code = "LN1148",
    tag = "misplaced-extra-member-in-deb",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MisplacedExtraMemberInDeb {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(members) = ctx.deb_members() else {
            return;
        };
        if !members.is_well_shaped() {
            return;
        }
        let positions = members.tarball_positions();
        let mut extras = Vec::new();
        for (position, member) in members.iter().enumerate().skip(1) {
            if Some(position) == positions.control || Some(position) == positions.data {
                continue;
            }
            let name = member.name();
            let text = match expected_extra_member_position(name) {
                Some(ExtraMemberPosition::Anywhere) => continue,
                Some(ExtraMemberPosition::At(expected)) if expected == position => continue,
                Some(ExtraMemberPosition::At(expected)) => {
                    format!("expected at position {expected}, but appeared")
                }
                None if name.starts_with('_') => "unexpected _member".to_string(),
                None => "unexpected member".to_string(),
            };
            extras.push(format!("{name} ({text} at position {position})"));
        }
        for extra in extras {
            ctx.diagnostic(Diagnostic::error(extra));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestCase};

    use super::*;

    fn members(names: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(TestCase::default()).deb_members(names.iter().copied())
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(TestCase::default()),
            members(&["debian-binary", "control.tar.gz", "data.tar.xz"]),
            // An uncompressed or zstd tarball is still a tarball.
            members(&["debian-binary", "control.tar", "data.tar.zst"]),
            // Not a well-shaped archive: `malformed-deb-archive`, not this Tag.
            members(&["debian-binary", "control.tar.xz"]),
            members(&["control.tar.xz", "debian-binary", "data.tar.xz", "extra"]),
            members(&[]),
        ];
        let fail = vec![
            members(&[
                "debian-binary",
                "control.tar.xz",
                "data.tar.xz",
                "extra-stuff",
            ]),
            members(&[
                "debian-binary",
                "_extra-stuff",
                "control.tar.xz",
                "data.tar.xz",
            ]),
            members(&[
                "debian-binary",
                "control.tar.xz",
                "_extra-stuff",
                "data.tar.xz",
            ]),
            // Without a Control tarball every member is an extra.
            members(&["debian-binary", "_a", "_b", "_c"]),
            // A udeb is checked the same way.
            members(&["debian-binary", "control.tar.xz", "data.tar.xz", "extra"]).udeb(),
        ];
        BinaryPackageTester::new(MisplacedExtraMemberInDeb, pass, fail).test_and_snapshot();
    }
}
