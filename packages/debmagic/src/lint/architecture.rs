//! Architecture restrictions of an override line (`[amd64 !i386]`), as lintian
//! 2.144.0 reads them: `Lintian::Data::Architectures` (`names`, `wildcards`,
//! `restriction_matches`) over the vendored `data/architectures/host.json`,
//! and the list test of `Lintian::Group::process`.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use serde::Deserialize;

#[derive(Deserialize)]
struct HostFile {
    host_variables: HashMap<String, HashMap<String, String>>,
}

/// Lintian's architecture tables, keyed the way its `Lintian::Data::Architectures`
/// builds them from `host_variables`.
#[derive(Default)]
struct Architectures {
    /// `names`: a spelling (`amd64`, or `linux-amd64` for the same host) to the
    /// hyphenated name of a host. Only used for the architecture of a processable.
    names: HashMap<String, String>,
    /// `wildcards`: a wildcard (`any-amd64`, `linux-any`, `any`) to the hyphenated
    /// names of the hosts it includes.
    wildcards: HashMap<String, HashSet<String>>,
}

static ARCHITECTURES: LazyLock<Architectures> =
    LazyLock::new(|| Architectures::from_host_json(include_str!("data/architectures/host.json")));

impl Architectures {
    fn from_host_json(json: &str) -> Self {
        let Ok(file) = serde_json::from_str::<HostFile>(json) else {
            return Self::default();
        };
        let mut architectures = Self::default();
        for (hyphenated, variables) in &file.host_variables {
            let field = |name: &str| variables.get(name).map_or("", String::as_str);
            architectures
                .names
                .insert(hyphenated.clone(), hyphenated.clone());
            if field("DEB_HOST_ARCH_OS") == "linux" {
                // Policy 11.1: `linux-amd64` and `amd64` are aliases.
                let alias = match hyphenated.strip_prefix("linux-") {
                    Some(short) => short.to_string(),
                    None => format!("linux-{hyphenated}"),
                };
                architectures.names.insert(alias, hyphenated.clone());
            }

            let abi = field("DEB_HOST_ARCH_ABI");
            let libc = field("DEB_HOST_ARCH_LIBC");
            let os = field("DEB_HOST_ARCH_OS");
            let cpu = field("DEB_HOST_ARCH_CPU");
            let wildcards = [
                "any".to_string(),
                "any-any".to_string(),
                format!("any-{cpu}"),
                format!("{os}-any"),
                "any-any-any".to_string(),
                format!("any-any-{cpu}"),
                format!("any-{os}-any"),
                format!("any-{os}-{cpu}"),
                format!("{libc}-any-any"),
                format!("{libc}-any-{cpu}"),
                format!("{libc}-{os}-any"),
                "any-any-any-any".to_string(),
                format!("any-any-any-{cpu}"),
                format!("any-any-{os}-any"),
                format!("any-any-{os}-{cpu}"),
                format!("any-{libc}-any-any"),
                format!("any-{libc}-any-{cpu}"),
                format!("any-{libc}-{os}-any"),
                format!("any-{libc}-{os}-{cpu}"),
                format!("{abi}-any-any-any"),
                format!("{abi}-any-any-{cpu}"),
                format!("{abi}-any-{os}-any"),
                format!("{abi}-any-{os}-{cpu}"),
                format!("{abi}-{libc}-any-any"),
                format!("{abi}-{libc}-any-{cpu}"),
                format!("{abi}-{libc}-{os}-any"),
            ];
            for wildcard in wildcards {
                architectures
                    .wildcards
                    .entry(wildcard)
                    .or_default()
                    .insert(hyphenated.clone());
            }
        }
        architectures
    }
}

/// Lintian's `restriction_matches` for one restriction whose leading `!` has
/// already been removed: the restriction is the architecture, or a wildcard that
/// includes it. The architecture may be spelt with its `linux-` alias; the
/// restriction may not (`[linux-amd64]` does not match `amd64`, as in lintian).
pub(crate) fn restriction_matches(restriction: &str, architecture: &str) -> bool {
    if restriction == architecture {
        return true;
    }
    let architectures = &*ARCHITECTURES;
    let host = architectures
        .names
        .get(architecture)
        .map_or(architecture, String::as_str);
    architectures
        .wildcards
        .get(restriction)
        .is_some_and(|hosts| hosts.contains(host))
}

/// Whether an override with the restriction list `restrictions` (as written,
/// `!` included) applies to a processable of architecture `architecture`.
/// An empty list applies everywhere. A list with a `!` entry applies only when no
/// entry matches; otherwise it applies when some entry matches.
pub(crate) fn restriction_list_keeps(restrictions: &[String], architecture: &str) -> bool {
    if restrictions.is_empty() {
        return true;
    }
    let negated = restrictions
        .iter()
        .any(|restriction| restriction.starts_with('!'));
    let some_matches = restrictions.iter().any(|restriction| {
        restriction_matches(
            restriction.strip_prefix('!').unwrap_or(restriction),
            architecture,
        )
    });
    some_matches != negated
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `(restriction, architecture, matches)` from lintian's
    /// `Lintian::Data::Architectures::restriction_matches` (lintian 2.144.0,
    /// `lib/Lintian/Data/Architectures.pm`, run with `perl -I lib` over its
    /// `data/architectures/host.json`).
    #[derive(Deserialize)]
    struct RestrictionCase(Vec<String>, String, bool);

    fn restriction_cases() -> Vec<RestrictionCase> {
        include_str!("data/override-pass/restriction-cases.jsonl")
            .lines()
            .map(|line| serde_json::from_str(line).expect("valid case line"))
            .collect()
    }

    #[test]
    fn host_json_loads() {
        let architectures = &*ARCHITECTURES;
        assert!(architectures.names.contains_key("amd64"));
        assert_eq!(
            architectures.names.get("linux-amd64").map(String::as_str),
            Some("amd64")
        );
        assert!(architectures.wildcards.contains_key("any"));
        assert!(architectures.wildcards.contains_key("linux-any"));
        assert!(architectures.wildcards["any-amd64"].contains("amd64"));
    }

    #[test]
    fn lintian_list_table() {
        let cases = restriction_cases();
        assert_eq!(cases.len(), 230);
        for case in &cases {
            let RestrictionCase(restrictions, architecture, keep) = case;
            assert_eq!(
                restriction_list_keeps(restrictions, architecture),
                *keep,
                "restrictions {restrictions:?} on {architecture}"
            );
        }
    }

    #[test]
    fn lintian_restriction_table() {
        // A single entry: `restriction_matches` with the `!` already removed.
        for RestrictionCase(restrictions, architecture, keep) in restriction_cases() {
            if let [restriction] = restrictions.as_slice()
                && !restriction.starts_with('!')
            {
                assert_eq!(
                    restriction_matches(restriction, &architecture),
                    keep,
                    "restriction {restriction} on {architecture}"
                );
            }
        }
    }

    #[test]
    fn empty_list_applies_everywhere() {
        assert!(restriction_list_keeps(&[], "amd64"));
        assert!(restriction_list_keeps(&[], "source"));
    }

    #[test]
    fn source_and_all_match_only_themselves() {
        assert!(restriction_matches("source", "source"));
        assert!(!restriction_matches("any", "source"));
        assert!(!restriction_matches("all", "amd64"));
        assert!(restriction_matches("all", "all"));
    }

    #[test]
    fn wildcard_alias_of_the_architecture() {
        assert!(restriction_matches("any", "linux-amd64"));
        assert!(restriction_matches("any-amd64", "linux-amd64"));
        assert!(!restriction_matches("any-arm", "linux-amd64"));
        assert!(!restriction_matches("linux-any", "source"));
    }
}
