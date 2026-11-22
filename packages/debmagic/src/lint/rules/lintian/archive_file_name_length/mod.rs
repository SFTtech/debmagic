//! Rules for lintian's `archive/file/name/length` Check.

use std::sync::LazyLock;

use regex::Regex;

mod ln1288_source_package_component_has_long_file_name;
mod ln1483_package_has_long_file_name;

pub(crate) use ln1288_source_package_component_has_long_file_name::SourcePackageComponentHasLongFileName;
pub(crate) use ln1483_package_has_long_file_name::PackageHasLongFileName;

/// Whether lintian's `package-has-long-file-name` length test fails for a file
/// name: the name without a `+salsaci+N+N` suffix, minus the length of the
/// architecture and plus the length of `kfreebsd-amd64` (lintian's longest
/// architecture), counted in characters, is over 80.
pub(super) fn file_name_too_long(basename: &str, architecture: &str) -> bool {
    static SALSACI: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\+salsaci\+\d+\+\d+(_[[:alnum:]]+\.[[:alnum:]]+)$").expect("valid regex")
    });
    const LONGEST_ARCHITECTURE: usize = 14;
    const FILENAME_LENGTH_LIMIT: usize = 80;
    let stripped = SALSACI.replace(basename, "${1}");
    let adjusted = stripped.chars().count() + LONGEST_ARCHITECTURE;
    let adjusted = adjusted.saturating_sub(architecture.chars().count());
    adjusted > FILENAME_LENGTH_LIMIT
}
