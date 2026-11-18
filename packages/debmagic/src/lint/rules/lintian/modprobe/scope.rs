//! Installed paths under the `modprobe` Check's directories.

use std::sync::LazyLock;

use regex::Regex;

static SCOPE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:usr/lib|etc)/(?:modprobe|modules-load)\.d/.").expect("valid regex")
});

/// A path below `usr/lib/` or `etc/` `modprobe.d` or `modules-load.d`. The
/// directory itself does not match.
pub(super) fn in_scope(path: &str) -> bool {
    SCOPE.is_match(path)
}
