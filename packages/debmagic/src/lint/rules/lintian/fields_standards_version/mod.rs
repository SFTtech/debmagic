//! Rules for lintian's `fields/standards-version` Check.
//!
//! The Standards-Version is read from the Dsc when there is one, otherwise from
//! the source paragraph of `debian/control`. Each Rule emits one Tag.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::LazyLock;

use crate::lint::context::SourceContext;

mod ln0725_timewarp_standards_version;
mod ln0774_invalid_standards_version;

pub(crate) use ln0725_timewarp_standards_version::TimewarpStandardsVersion;
pub(crate) use ln0774_invalid_standards_version::InvalidStandardsVersion;

/// Policy releases and their epochs, from lintian 2.139.0's
/// `debian-policy/releases.json`.
const POLICY_RELEASES: &str = include_str!("debian_policy_releases");

struct PolicyReleases {
    max_dots: usize,
    latest: String,
    epochs: HashMap<String, i64>,
}

static RELEASES: LazyLock<PolicyReleases> = LazyLock::new(|| {
    let rows: Vec<(&str, i64)> = crate::lint::data::lines(POLICY_RELEASES)
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let version = fields.next()?;
            let epoch = fields.next()?.parse().ok()?;
            Some((version, epoch))
        })
        .collect();
    let max_dots = rows
        .iter()
        .map(|(version, _)| dots(version))
        .max()
        .unwrap_or(0);
    // The table is newest first.
    let latest = rows
        .first()
        .map(|(version, _)| version.to_string())
        .unwrap_or_default();
    let epochs = rows
        .into_iter()
        .map(|(version, epoch)| (normalize(version, max_dots), epoch))
        .collect();
    PolicyReleases {
        max_dots,
        latest,
        epochs,
    }
});

fn dots(version: &str) -> usize {
    version.matches('.').count()
}

/// Pads with `.0` up to the dot count of the longest release.
fn normalize(version: &str, max_dots: usize) -> String {
    let mut normalized = version.to_string();
    for _ in dots(version)..max_dots {
        normalized.push_str(".0");
    }
    normalized
}

/// Epoch of a known Policy release. `None` when the version is not one.
pub(super) fn policy_release_epoch(version: &str) -> Option<i64> {
    let releases = &*RELEASES;
    releases
        .epochs
        .get(&normalize(version, releases.max_dots))
        .copied()
}

/// The Standards-Version of a Source package (its Dsc) or of a Source tree
/// (`debian/control`). `None` when not declared.
pub(super) fn standards_version(ctx: &SourceContext<'_>) -> Option<String> {
    match ctx.dsc() {
        Some(dsc) => dsc.fields().unfolded_value("Standards-Version"),
        None => ctx
            .debian_control()?
            .source()
            .unfolded_value("Standards-Version"),
    }
}

/// Three or more dot-separated numbers, trailing dots dropped as Perl's
/// `split` does. Lintian's invalid-version test.
pub(super) fn is_three_part_number(version: &str) -> bool {
    let components: Vec<&str> = version.trim_end_matches('.').split('.').collect();
    components.len() >= 3
        && components
            .iter()
            .all(|component| !component.is_empty() && component.bytes().all(|b| b.is_ascii_digit()))
}

/// The newest Policy release, as the table lists it.
pub(super) fn latest_policy_version() -> &'static str {
    &RELEASES.latest
}

/// Whether `version` is a Policy release in the table.
pub(super) fn is_policy_release(version: &str) -> bool {
    policy_release_epoch(version).is_some()
}

/// Whether dotted-number `version` sorts after the newest Policy release.
pub(super) fn is_newer_than_latest_policy(version: &str) -> bool {
    dotted_cmp(version, latest_policy_version()) == Ordering::Greater
}

/// Compares dotted numbers component by component. A longer version sorts
/// after one it extends, as Sort::Versions does.
fn dotted_cmp(left: &str, right: &str) -> Ordering {
    let mut left = left.split('.');
    let mut right = right.split('.');
    loop {
        match (left.next(), right.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(a), Some(b)) => {
                let a = a.trim_start_matches('0');
                let b = b.trim_start_matches('0');
                match a.len().cmp(&b.len()).then_with(|| a.cmp(b)) {
                    Ordering::Equal => {}
                    other => return other,
                }
            }
        }
    }
}
