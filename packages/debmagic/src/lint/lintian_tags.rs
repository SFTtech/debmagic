//! Every Tag name lintian 2.139.0 knows, and the old names it still accepts.
//!
//! Vendored from `tags/**/*.tag` (see `data/tags/`). This is lintian's Tag
//! catalogue, not ours: a Tag here need not be a Rule.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

static KNOWN: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| crate::lint::data::lines(include_str!("data/tags/known-tags")).collect());

static RENAMED: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("data/tags/renamed-tags"))
        .filter_map(|line| {
            let (old, current) = line.split_once("=>")?;
            Some((old.trim(), current.trim()))
        })
        .collect()
});

static DURABLE: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| crate::lint::data::lines(include_str!("data/tags/show-always")).collect());

/// Whether lintian declares the Tag `Show-Always: yes` (by current name). An
/// override cannot suppress such a Tag (ADR 0057).
pub(crate) fn is_durable(current_name: &str) -> bool {
    DURABLE.contains(current_name)
}

/// Current name of a lintian Tag, as lintian's `Profile::get_current_name`:
/// an old name maps to the Tag that renamed it, a current name to itself.
/// `None` when lintian 2.139.0 knows neither.
pub(crate) fn current_tag_name(name: &str) -> Option<&'static str> {
    if let Some(current) = RENAMED.get(name) {
        return Some(current);
    }
    KNOWN.get(name).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_name_is_itself() {
        assert_eq!(current_tag_name("alien-tag"), Some("alien-tag"));
        assert_eq!(
            current_tag_name("copyright-contains-dh_make-todo-boilerplate"),
            Some("copyright-contains-dh_make-todo-boilerplate")
        );
    }

    #[test]
    fn name_spaced_tag_carries_its_check() {
        assert_eq!(
            current_tag_name("team/pkg-perl/vcs/no-git"),
            Some("team/pkg-perl/vcs/no-git")
        );
        assert_eq!(current_tag_name("no-git"), None);
    }

    #[test]
    fn old_name_maps_to_current() {
        assert_eq!(
            current_tag_name("package-install-apt-sources"),
            Some("package-installs-apt-sources")
        );
        assert_eq!(
            current_tag_name("manpage-has-bad-whatis-entry"),
            Some("bad-whatis-entry")
        );
    }

    #[test]
    fn unknown_name_is_none() {
        assert_eq!(current_tag_name("foo"), None);
        assert_eq!(current_tag_name("lintian-override:"), None);
        assert_eq!(current_tag_name(""), None);
    }

    #[test]
    fn catalogue_is_complete() {
        assert_eq!(KNOWN.len(), 1529);
        assert_eq!(RENAMED.len(), 221);
        for current in RENAMED.values() {
            assert!(KNOWN.contains(current), "{current} is not a known Tag");
        }
    }

    #[test]
    fn show_always_tags_are_known_and_durable() {
        assert_eq!(DURABLE.len(), 7);
        for name in DURABLE.iter() {
            assert!(KNOWN.contains(name), "{name} is not a known Tag");
            assert!(is_durable(name));
        }
        assert!(is_durable("unused-override"));
        assert!(!is_durable("cute-field"));
        assert!(!is_durable("package-installs-apt-sources"));
    }
}
