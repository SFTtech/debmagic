//! Rules for lintian's `fields/distribution` Check.

use std::sync::LazyLock;

use regex::Regex;

mod ln1480_bad_distribution_in_changes_file;
mod ln1481_backports_changes_missing;

pub(crate) use ln1480_bad_distribution_in_changes_file::BadDistributionInChangesFile;
pub(crate) use ln1481_backports_changes_missing::BackportsChangesMissing;

/// The distribution with the common extensions stripped (`stable-security`
/// reads as `stable`; `sid` and `experimental` are left alone). Lintian's
/// `$major`. The longest suffix wins, as the leftmost match of lintian's
/// anchored regular expression does.
pub(super) fn major_distribution(target: &str) -> &str {
    const EXTENSIONS: [&str; 10] = [
        "-backports-sloppy",
        "-backports-staging",
        "-proposed-updates",
        "-backports",
        "-lts",
        "-proposed",
        "-updates",
        "-security",
        "-volatile",
        "-fasttrack",
    ];
    EXTENSIONS
        .iter()
        .find_map(|extension| target.strip_suffix(extension))
        .unwrap_or(target)
}

/// The `(distnumber, bpoversion)` digits of a `~bpoN+M` version (with the
/// optional `+salsaci` tail), as lintian's regular expression captures them.
/// `None` when the version has no backports suffix.
pub(super) fn bpo_numbers(version: &str) -> Option<(&str, &str)> {
    static BPO: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"~bpo(\d+)\+(\d+)(\+salsaci(\+\d+)*)?$").expect("valid regex")
    });
    let captures = BPO.captures(version)?;
    Some((captures.get(1)?.as_str(), captures.get(2)?.as_str()))
}

/// The value of a run of digits as lintian's numeric comparison sees it: a
/// number too large for `u64` is larger than any small number, so it saturates.
pub(super) fn digits_value(digits: &str) -> u64 {
    digits.bytes().fold(0u64, |value, digit| {
        value
            .saturating_mul(10)
            .saturating_add(u64::from(digit - b'0'))
    })
}

/// How many entries the `Changes` field of a `.changes` holds: lines of the
/// form `source (version) distribution`, as lintian counts them to tell a
/// single changelog entry apart.
pub(super) fn changes_entry_count(changes: &str) -> usize {
    static ENTRY: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?: \.)?\s*\S+\s+\(([^()]+)\)\s+\S+").expect("valid regex")
    });
    ENTRY.find_iter(changes).count()
}
