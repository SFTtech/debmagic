//! `data/fields/obsolete-packages`, `name` or `name => replacement`.

use std::sync::LazyLock;

static PACKAGES: LazyLock<Vec<(String, String)>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/fields/obsolete-packages"))
        .map(|line| {
            let (name, replacement) = line
                .split_once("=>")
                .map(|(name, replacement)| (name.trim(), replacement.trim()))
                .unwrap_or((line, ""));
            (name.to_string(), replacement.to_string())
        })
        .collect()
});

pub(super) fn replacement(package: &str) -> Option<&str> {
    PACKAGES
        .iter()
        .find(|(name, _)| name == package)
        .map(|(_, replacement)| replacement.as_str())
}

/// Emacs versions lintian still warns about as a primary dependency.
pub(super) fn obsolete_emacs(package: &str) -> bool {
    const VERSIONS: &[&str] = &["21", "22", "23"];
    const FLAVORS: &[&str] = &["", "-el", "-gtk", "-nox", "-lucid"];
    VERSIONS.iter().any(|version| {
        FLAVORS
            .iter()
            .any(|flavor| package == format!("emacs{version}{flavor}"))
    })
}
