//! `data/fields/metapackages`: convenience packages that must not be depended on.

use std::sync::LazyLock;

static PACKAGES: LazyLock<Vec<String>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/fields/metapackages"))
        .map(str::to_string)
        .collect()
});

pub(super) fn is_known_metapackage(package: &str) -> bool {
    PACKAGES.iter().any(|name| name == package)
}
