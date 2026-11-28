//! Every Tag name lintian 2.144.0 knows, and the old names it still accepts.
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
/// override cannot suppress such a Tag.
pub(crate) fn is_durable(current_name: &str) -> bool {
    DURABLE.contains(current_name)
}

/// Current name of a lintian Tag, as lintian's `Profile::get_current_name`:
/// an old name maps to the Tag that renamed it, a current name to itself.
/// `None` when lintian 2.144.0 knows neither.
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
        assert_eq!(KNOWN.len(), 1535);
        assert_eq!(RENAMED.len(), 225);
        for current in RENAMED.values() {
            assert!(KNOWN.contains(current), "{current} is not a known Tag");
        }
    }

    /// Lintian's `Profile::get_current_name` checks the old names first, so a
    /// name that is both a Tag and an old name would never resolve as a Tag.
    #[test]
    fn old_names_are_not_current_names() {
        for old in RENAMED.keys() {
            assert!(!KNOWN.contains(old), "{old} is both a Tag and an old name");
        }
    }

    /// Every old name resolves to the Tag that carries it now, and that Tag
    /// resolves to itself, so an override naming either reaches the same Rule.
    #[test]
    fn every_renamed_tag_resolves_to_a_known_current_name() {
        for (old, current) in RENAMED.iter() {
            assert_eq!(current_tag_name(old), Some(*current));
            assert_eq!(current_tag_name(current), Some(*current));
        }
    }

    /// Renames that lintian made after the catalogue was first vendored (2.139.0):
    /// an override written against the old name still reaches its Tag.
    #[test]
    fn renames_added_after_2_139_resolve_to_their_current_name() {
        let renames = [
            (
                "debhelper-compat-file-is-missing",
                "debhelper-base-compat-level-is-missing",
            ),
            (
                "declare-requested-python-versions-for-test",
                "declare-python-versions-for-test",
            ),
            ("exclusive-runtime-tests-field", "conflicting-test-fields"),
            (
                "systemd-tmpfiles.d-outside-usr-lib",
                "misplaced-systemd-tmpfiles",
            ),
        ];
        for (old, current) in renames {
            assert_eq!(current_tag_name(old), Some(current));
            assert!(KNOWN.contains(current), "{current} is not a known Tag");
        }
    }

    /// Every LN Rule reports a current lintian Tag, not an old name. A
    /// name-spaced Tag is declared under its bare name (`no-git` for
    /// `team/pkg-perl/vcs/no-git`), so a Rule's Tag is a known Tag or the bare
    /// name of a name-spaced known Tag (`no-git` is one under two checks, so
    /// the Rule's check is not recoverable from its Tag). The native DM Rules
    /// have no lintian Tag and are skipped.
    #[test]
    fn every_rule_tag_is_a_current_lintian_tag() {
        for rule in crate::lint::registry::all_rules() {
            if !rule.code().as_str().starts_with("LN") {
                continue;
            }
            let tag = rule.tag().0;
            let name_spaced = KNOWN
                .iter()
                .any(|known| known.rsplit_once('/').is_some_and(|(_, bare)| bare == tag));
            assert!(
                KNOWN.contains(tag) || name_spaced,
                "{tag} is not a known Tag"
            );
            assert!(!RENAMED.contains_key(tag), "{tag} is an old name");
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
