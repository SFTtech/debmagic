//! Rules for lintian's `fields/standards-version` Check.
//!
//! The Standards-Version is read from the Dsc when there is one, otherwise from
//! the source paragraph of `debian/control`. Each Rule emits one Tag.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::lint::context::SourceContext;

mod ln0725_timewarp_standards_version;

pub(crate) use ln0725_timewarp_standards_version::TimewarpStandardsVersion;

/// Policy releases and their epochs, from lintian 2.139.0's
/// `debian-policy/releases.json`.
const POLICY_RELEASES: &str = include_str!("debian_policy_releases");

struct PolicyReleases {
    max_dots: usize,
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
    let epochs = rows
        .into_iter()
        .map(|(version, epoch)| (normalize(version, max_dots), epoch))
        .collect();
    PolicyReleases { max_dots, epochs }
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
