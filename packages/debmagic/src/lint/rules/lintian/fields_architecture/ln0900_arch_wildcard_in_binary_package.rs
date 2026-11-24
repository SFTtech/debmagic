use std::collections::HashSet;
use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct ArchWildcardInBinaryPackage;

declare_rule! {
    /// An `Architecture` value in a binary package that is a wildcard.
    ///
    /// Each whitespace-separated value of the Installation control
    /// `Architecture` field is checked against the wildcards lintian derives
    /// from the vendored `data/architectures/host.json`: `any`, `<os>-any`,
    /// `any-<cpu>`, and every ABI, libc, OS, and CPU combination with `any`
    /// in its place (26 shapes per architecture). `any` itself is a wildcard.
    /// Extra is the offending value. There is no Location. Applies to deb,
    /// udeb, and ddeb.
    ArchWildcardInBinaryPackage,
    code = "LN0900",
    tag = "arch-wildcard-in-binary-package",
    default_selected = true,
    experimental = false,
}

static WILDCARDS: LazyLock<HashSet<String>> =
    LazyLock::new(|| wildcards(include_str!("../../../data/architectures/host.json")));

impl BinaryPackageRule for ArchWildcardInBinaryPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let architectures: Vec<String> = ctx
            .installation_control()
            .fields()
            .get("Architecture")
            .map(|value| value.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default();
        for architecture in architectures {
            if WILDCARDS.contains(&architecture) {
                ctx.diagnostic(Diagnostic::error(architecture));
            }
        }
    }
}

/// Lintian's `Lintian::Data::Architectures::wildcards`: the key set of the
/// map built from `host_variables`.
fn wildcards(json: &str) -> HashSet<String> {
    let mut found = HashSet::new();
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return found;
    };
    let Some(hosts) = value
        .get("host_variables")
        .and_then(|hosts| hosts.as_object())
    else {
        return found;
    };
    for variables in hosts.values() {
        let field = |name: &str| {
            variables
                .get(name)
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string()
        };
        let abi = field("DEB_HOST_ARCH_ABI");
        let libc = field("DEB_HOST_ARCH_LIBC");
        let os = field("DEB_HOST_ARCH_OS");
        let cpu = field("DEB_HOST_ARCH_CPU");
        let patterns = [
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
        found.extend(patterns);
    }
    found
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn arch(architecture: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Architecture", architecture)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            arch("all"),
            arch("amd64"),
            arch("amd64 arm64"),
            arch("linux-amd64"),
            arch("source"),
            // Not a derived wildcard shape.
            arch("foo-bar"),
        ];
        let fail = vec![
            arch("any"),
            arch("any-i386"),
            arch("any-amd64"),
            arch("linux-any"),
            arch("amd64 any-arm64"),
            arch("any-any"),
        ];
        BinaryPackageTester::new(ArchWildcardInBinaryPackage, pass, fail).test_and_snapshot();
    }
}
