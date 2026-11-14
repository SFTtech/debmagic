//! `data/fields/essential`: packages that are always installed.

use std::sync::LazyLock;

static PACKAGES: LazyLock<Vec<String>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/fields/essential"))
        .map(str::to_string)
        .collect()
});

pub(super) fn is_essential(package: &str) -> bool {
    PACKAGES.iter().any(|name| name == package)
}
