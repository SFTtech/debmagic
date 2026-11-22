//! Rules for lintian's `fields/distribution` Check.

mod ln1480_bad_distribution_in_changes_file;

pub(crate) use ln1480_bad_distribution_in_changes_file::BadDistributionInChangesFile;

/// The distribution with the common extensions stripped (`stable-security`
/// reads as `stable`; `sid` and `experimental` are left alone). Lintian's
/// `$major`. The longest suffix wins, as the leftmost match of lintian's
/// anchored regular expression does.
#[allow(dead_code)]
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
