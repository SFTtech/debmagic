//! `data/fields/virtual-packages`, one key per line.
//!
//! Lintian stores the whole line. A versioned entry such as
//! `vim (= 2:9.0.0242-1)` does not match the package name `vim`.

use std::collections::HashSet;
use std::sync::LazyLock;

static PACKAGES: LazyLock<HashSet<String>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/fields/virtual-packages"))
        .map(str::to_string)
        .collect()
});

pub(super) fn is_virtual(package: &str) -> bool {
    PACKAGES.contains(package)
}
