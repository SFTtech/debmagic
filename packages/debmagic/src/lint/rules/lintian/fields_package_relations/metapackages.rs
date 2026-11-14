//! `data/fields/metapackages`, one package name per line.

use std::collections::HashSet;
use std::sync::LazyLock;

static PACKAGES: LazyLock<HashSet<String>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/fields/metapackages"))
        .map(str::to_string)
        .collect()
});

pub(super) fn is_metapackage(package: &str) -> bool {
    PACKAGES.contains(package)
}
