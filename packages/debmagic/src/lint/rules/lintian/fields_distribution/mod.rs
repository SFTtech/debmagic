//! Rules for lintian's `fields/distribution` Check.

mod ln1493_multiple_distributions_in_changes_file;
mod ln1496_unreleased_changes;
mod ln1497_upload_has_backports_version_number;

pub(crate) use ln1493_multiple_distributions_in_changes_file::MultipleDistributionsInChangesFile;
pub(crate) use ln1496_unreleased_changes::UnreleasedChanges;
pub(crate) use ln1497_upload_has_backports_version_number::UploadHasBackportsVersionNumber;
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::changes::Changes;

mod ln1480_bad_distribution_in_changes_file;
mod ln1481_backports_changes_missing;
mod ln1482_backports_upload_has_incorrect_version_number;
mod ln1487_distribution_and_changes_mismatch;
mod ln1488_distribution_and_experimental_mismatch;

pub(crate) use ln1480_bad_distribution_in_changes_file::BadDistributionInChangesFile;
pub(crate) use ln1481_backports_changes_missing::BackportsChangesMissing;
pub(crate) use ln1482_backports_upload_has_incorrect_version_number::BackportsUploadHasIncorrectVersionNumber;
pub(crate) use ln1487_distribution_and_changes_mismatch::DistributionAndChangesMismatch;
pub(crate) use ln1488_distribution_and_experimental_mismatch::DistributionAndExperimentalMismatch;

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

/// What lintian's Check compares: the words of `Distribution` and the words of
/// the changelog distributions in the `Changes` field, each side without the
/// other's words.
pub(super) struct DistributionMismatch {
    pub(super) from_distribution: Vec<String>,
    pub(super) from_changes: Vec<String>,
}

/// Lintian's comparison of the `Distribution` field with the distributions of
/// the first changelog entry in the `Changes` field. `sid` reads as `unstable`
/// on both sides. `None` when the Check has nothing to compare (no changelog
/// distribution), when it returns early for `UNRELEASED` changes that the
/// `Distribution` does not name, or when the two agree.
pub(super) fn distribution_mismatch(changes: &Changes) -> Option<DistributionMismatch> {
    static FIRST_ENTRY: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s*\S+\s+\([^()]+\)([^;]+);").expect("valid regex"));
    let normalise = |word: &str| {
        if word == "sid" {
            "unstable".to_string()
        } else {
            word.to_string()
        }
    };
    let text = changes
        .fields()
        .untrimmed_value("Changes")
        .unwrap_or("")
        .trim_start();
    let changelog: Vec<String> = FIRST_ENTRY
        .captures(text)
        .and_then(|captures| captures.get(1))
        .map(|words| words.as_str().split_whitespace().map(normalise).collect())
        .unwrap_or_default();
    if changelog.is_empty() {
        return None;
    }
    let distribution: Vec<String> = changes.distributions().into_iter().map(normalise).collect();
    if changelog.iter().any(|word| word == "UNRELEASED")
        && !distribution.iter().any(|word| word == "UNRELEASED")
    {
        return None;
    }
    let from_distribution: Vec<String> = distribution
        .iter()
        .filter(|word| !changelog.contains(word))
        .cloned()
        .collect();
    let from_changes: Vec<String> = changelog
        .iter()
        .filter(|word| !distribution.contains(word))
        .cloned()
        .collect();
    (!from_distribution.is_empty() || !from_changes.is_empty()).then_some(DistributionMismatch {
        from_distribution,
        from_changes,
    })
}
