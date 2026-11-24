use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::deb_members::{DebMember, DebMembers, Tarball, official_extension};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct MalformedDebArchive;

declare_rule! {
    /// The `.deb` is not a correctly constructed archive: fewer than three
    /// ar members, a first member other than `debian-binary`, a Control or
    /// Data tarball that is missing or has a name other than an official one
    /// (`control.tar` or `data.tar` with an extension from lintian's
    /// `deb-format` compression tables), or a `debian-binary` whose version is
    /// not 2.x. Each problem is one hint, in lintian's words and order. The
    /// Subject is the location; the Extra is the problem.
    ///
    /// Checked with the members' names. The version is read from the kept
    /// `debian-binary` contents, and is skipped when they are not kept (a
    /// fixture). lintian's `ar` error branch is not reachable here: a file
    /// that is not an ar archive is not a Binary package at all.
    MalformedDebArchive,
    code = "LN1304",
    tag = "malformed-deb-archive",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MalformedDebArchive {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(members) = ctx.deb_members() else {
            return;
        };
        for extra in malformed_hints(members) {
            ctx.diagnostic(Diagnostic::error(extra));
        }
    }
}

/// The `malformed-deb-archive` hints for the ar members, in lintian's order.
fn malformed_hints(members: &DebMembers) -> Vec<String> {
    let names: Vec<&str> = members.iter().map(DebMember::name).collect();
    let count = names.len();
    let mut hints = Vec::new();
    let (control, data) = if count < 3 {
        hints.push(format!("found only {count} members instead of 3"));
        (None, None)
    } else if names[0] != "debian-binary" {
        hints.push(format!("first member {} not debian-binary", names[0]));
        (None, None)
    } else if count == 3 && names.iter().all(|name| !name.starts_with('_')) {
        (Some(names[1]), Some(names[2]))
    } else {
        let positions = members.tarball_positions();
        (
            positions.control.map(|index| names[index]),
            positions.data.map(|index| names[index]),
        )
    };
    let mut failed = false;

    match control {
        None => {
            hints.push("Missing control.tar member".to_string());
            failed = true;
        }
        Some(name) if official_extension(Tarball::Control, name).is_none() => {
            hints.push(format!(
                "second (official) member {name} not control.tar.({})",
                pattern(Tarball::Control)
            ));
            failed = true;
        }
        Some(_) => {}
    }

    match data {
        None => {
            hints.push("Missing data.tar member".to_string());
            failed = true;
        }
        Some(name) if official_extension(Tarball::Data, name).is_none() => {
            hints.push(format!(
                "third (official) member {name} not data.tar.({})",
                pattern(Tarball::Data)
            ));
            failed = true;
        }
        Some(_) => {}
    }

    if !failed
        && let Some(contents) = members.debian_binary().and_then(DebMember::contents)
        && let Some(hint) = version_hint(contents)
    {
        hints.push(hint);
    }
    hints
}

/// The compression extensions of a tarball, joined by `|` as lintian prints them.
fn pattern(tarball: Tarball) -> String {
    tarball.compressions().join("|")
}

/// The hint for a `debian-binary` that is not `2.<digits>` followed by a
/// newline; its first line is the version named. `None` when it is 2.x.
fn version_hint(contents: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(contents);
    if is_format_two(&text) {
        return None;
    }
    let version = text.split('\n').next().unwrap_or("");
    Some(format!("version {version} not 2.0"))
}

/// lintian's `^2\.\d+\n`.
fn is_format_two(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("2.") else {
        return false;
    };
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    digits > 0 && rest[digits..].starts_with('\n')
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
            members(&["debian-binary", "control.tar.xz", "data.tar.bz2"]),
            members(&["debian-binary", "control.tar.xz", "data.tar"]),
            members(&[
                "debian-binary",
                "_before",
                "control.tar.xz",
                "_between",
                "data.tar.xz",
            ]),
        ];
        let fail = vec![
            members(&["debian-binary", "control.tar"]),
            members(&["debian-binary", "control.tar.xz"]),
            members(&["control.tar.xz", "debian-binary", "data.tar.xz"]),
            members(&["debian-binary", "control.tar.xz", "data.tar.lzma"]),
            members(&["debian-binary", "control.tar.zst", "data.tar.xz"]),
            members(&["debian-binary", "control.tar.bz2", "data.tar.xz"]),
            members(&["debian-binary", "control.tar.xz", "data.tar.Z"]),
            members(&["debian-binary", "_before", "control.tar.xz"]),
        ];
        BinaryPackageTester::new(MalformedDebArchive, pass, fail).test_and_snapshot();
    }

    #[test]
    fn hints_for_each_malformed_shape() {
        let hints =
            |names: &[&str]| malformed_hints(&DebMembers::from_names(names.iter().copied()));
        assert_eq!(
            hints(&["debian-binary"]),
            [
                "found only 1 members instead of 3",
                "Missing control.tar member",
                "Missing data.tar member"
            ]
        );
        assert_eq!(
            hints(&["control.tar.xz", "debian-binary", "data.tar.xz"]),
            [
                "first member control.tar.xz not debian-binary",
                "Missing control.tar member",
                "Missing data.tar member"
            ]
        );
        assert_eq!(
            hints(&["debian-binary", "control.tar.xz", "data.tar.lzma"]),
            ["third (official) member data.tar.lzma not data.tar.(gz|xz|bz2)"]
        );
        assert_eq!(
            hints(&["debian-binary", "control.tar.zst", "data.tar.xz"]),
            ["second (official) member control.tar.zst not control.tar.(gz|xz)"]
        );
        assert!(hints(&["debian-binary", "control.tar.xz", "data.tar.xz"]).is_empty());
    }

    #[test]
    fn version_must_be_two_point_digits_and_newline() {
        assert_eq!(version_hint(b"2.0\n"), None);
        assert_eq!(version_hint(b"2.10\n"), None);
        assert_eq!(
            version_hint(b"3.0\n"),
            Some("version 3.0 not 2.0".to_string())
        );
        assert_eq!(
            version_hint(b"2.0"),
            Some("version 2.0 not 2.0".to_string())
        );
        assert_eq!(
            version_hint(b"2.x\n"),
            Some("version 2.x not 2.0".to_string())
        );
        assert_eq!(version_hint(b""), Some("version  not 2.0".to_string()));
    }
}
