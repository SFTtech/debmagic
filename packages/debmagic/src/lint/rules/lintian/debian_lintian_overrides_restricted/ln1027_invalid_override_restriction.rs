use std::collections::HashSet;
use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct InvalidOverrideRestriction;

declare_rule! {
    /// An architecture restriction `[…]` in the installed override file
    /// (`usr/share/lintian/overrides/<package>`, else `.gz`) is invalid.
    ///
    /// Lintian's messages, one Diagnostic each, per override line in file
    /// order: `Architecture list in Arch:all installable` when the Subject's
    /// `Architecture` is `all` and the override has a restriction; then
    /// `Unknown architecture wildcard <value>` for each restriction value
    /// (leading `!` removed) that is neither a release architecture, a derived
    /// wildcard, nor `all`; and `Inconsistent architecture negation` when some
    /// but not all values are negated. Only checks after an invalid value are
    /// skipped, as in lintian. The override's line is the Location. Lines that
    /// do not parse are left to `malformed-override`. Overrides are not
    /// applied. Applies to deb, udeb, and ddeb.
    InvalidOverrideRestriction,
    code = "LN1027",
    tag = "invalid-override-restriction",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InvalidOverrideRestriction {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(overrides) = ctx.lintian_overrides() else {
            return;
        };
        let architecture = ctx
            .installation_control()
            .fields()
            .get("Architecture")
            .unwrap_or_default()
            .to_string();
        let path = overrides.path().to_path_buf();
        for item in overrides.overrides() {
            let location = || Location {
                path: path.clone(),
                line: Some(item.line()),
                column: None,
            };
            let restrictions = item.architectures();
            if !restrictions.is_empty() && architecture == "all" {
                ctx.diagnostic(
                    Diagnostic::error("Architecture list in Arch:all installable")
                        .with_location(location()),
                );
                continue;
            }
            let invalid: Vec<&String> = restrictions
                .iter()
                .filter(|restriction| !valid_restriction(restriction))
                .collect();
            for restriction in &invalid {
                ctx.diagnostic(
                    Diagnostic::error(format!("Unknown architecture wildcard {restriction}"))
                        .with_location(location()),
                );
            }
            if !invalid.is_empty() {
                continue;
            }
            let negations = restrictions
                .iter()
                .filter(|restriction| restriction.starts_with('!'))
                .count();
            if negations > 0 && negations != restrictions.len() {
                ctx.diagnostic(
                    Diagnostic::error("Inconsistent architecture negation")
                        .with_location(location()),
                );
            }
        }
    }
}

/// Lintian's `Lintian::Data::Architectures::valid_restriction`: a release
/// architecture, a derived wildcard, or `all`, after a leading `!` is removed.
fn valid_restriction(restriction: &str) -> bool {
    let name = restriction.strip_prefix('!').unwrap_or(restriction);
    RELEASE_NAMES.contains(name) || WILDCARDS.contains(name) || name == "all"
}

static RELEASE_NAMES: LazyLock<HashSet<String>> =
    LazyLock::new(|| release_names(include_str!("../../../data/architectures/host.json")));

static WILDCARDS: LazyLock<HashSet<String>> =
    LazyLock::new(|| wildcards(include_str!("../../../data/architectures/host.json")));

/// Lintian's `Lintian::Data::Architectures::names`: the key set of the map
/// built from `host_variables`, with the `linux-` aliases of Policy 11.1.
fn release_names(json: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    for (hyphenated, variables) in hosts(json) {
        names.insert(hyphenated.clone());
        if field(&variables, "DEB_HOST_ARCH_OS") != "linux" {
            continue;
        }
        match hyphenated.strip_prefix("linux-") {
            Some(short) => names.insert(short.to_string()),
            None => names.insert(format!("linux-{hyphenated}")),
        };
    }
    names
}

/// Lintian's `Lintian::Data::Architectures::wildcards`: the key set of the
/// map built from `host_variables`.
fn wildcards(json: &str) -> HashSet<String> {
    let mut found = HashSet::new();
    for (_, variables) in hosts(json) {
        let abi = field(&variables, "DEB_HOST_ARCH_ABI");
        let libc = field(&variables, "DEB_HOST_ARCH_LIBC");
        let os = field(&variables, "DEB_HOST_ARCH_OS");
        let cpu = field(&variables, "DEB_HOST_ARCH_CPU");
        found.extend([
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
        ]);
    }
    found
}

/// `(hyphenated name, DEB_HOST_* variables)` of every host in `host.json`.
fn hosts(json: &str) -> Vec<(String, serde_json::Value)> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    value
        .get("host_variables")
        .and_then(|hosts| hosts.as_object())
        .map(|hosts| {
            hosts
                .iter()
                .map(|(name, variables)| (name.clone(), variables.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn field(variables: &serde_json::Value, name: &str) -> String {
    variables
        .get(name)
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn overrides(architecture: &str, text: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/share/lintian/overrides/example".to_string(),
            TestFile::new(text),
        )])
        .field("Architecture", architecture)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            overrides("amd64", "example: cute-field\n"),
            overrides("amd64", "[amd64]: cute-field\n"),
            overrides("amd64", "[amd64 arm64]: cute-field\n"),
            overrides("amd64", "[linux-amd64]: cute-field\n"),
            overrides("amd64", "[!amd64 !i386]: cute-field\n"),
            overrides("amd64", "[any linux-any]: cute-field\n"),
            overrides("amd64", "[all]: cute-field\n"),
            overrides("amd64", "[any-any-amd64]: cute-field\n"),
            overrides("all", "example: cute-field\n"),
            overrides("amd64", "[any-amd64 arm64]: cute-field\n").udeb(),
        ];
        let fail = vec![
            overrides("amd64", "[any-amd64 !i386]: cute-field\n"),
            overrides("amd64", "[amd64 !i386]: cute-field\n").udeb(),
            overrides("all", "[amd64]: cute-field\n"),
            overrides("all", "[any]: cute-field\n"),
            overrides("amd64", "[bingo]: cute-field\n"),
            overrides("amd64", "[bingo !amd64]: cute-field\n"),
            overrides("amd64", "[amd64 !i386 bingo]: cute-field\n"),
            overrides("amd64", "[amd64 !i386]: a\n[!arm64 bingo]: b\n"),
            overrides("amd64", "[!amd64 i386]: cute-field\n"),
            overrides("amd64", "[bingo]: cute-field\n").udeb(),
        ];
        BinaryPackageTester::new(InvalidOverrideRestriction, pass, fail).test_and_snapshot();
    }
}
