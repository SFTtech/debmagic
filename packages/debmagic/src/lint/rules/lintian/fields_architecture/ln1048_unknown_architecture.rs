use std::collections::HashSet;
use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct UnknownArchitecture;

declare_rule! {
    /// A binary package's `Architecture` value is not a known architecture.
    ///
    /// Each whitespace-separated value of the Installation control
    /// `Architecture` field, in order, is checked against the vendored
    /// `data/architectures/host.json`. A value passes when it is a Debian
    /// release architecture (a host name, or its `linux-` alias when the OS is
    /// Linux), a wildcard (the shapes `ArchWildcardInBinaryPackage` derives,
    /// `any` included), or `all`. Anything else is reported as a warning.
    /// `source` is reported here: it is only valid in `.changes` and
    /// `.buildinfo` files. Extra is the offending value and there is no
    /// Location. Applies to deb, udeb, and ddeb.
    UnknownArchitecture,
    code = "LN1048",
    tag = "unknown-architecture",
    default_selected = true,
    experimental = false,
}

const HOST_JSON: &str = include_str!("../../../data/architectures/host.json");

/// Known architecture names: lintian's `names` and `wildcards` tables.
struct Known {
    names: HashSet<String>,
    wildcards: HashSet<String>,
}

static KNOWN: LazyLock<Known> = LazyLock::new(|| known(HOST_JSON));

impl BinaryPackageRule for UnknownArchitecture {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let architectures: Vec<String> = ctx
            .installation_control()
            .fields()
            .get("Architecture")
            .map(|value| value.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default();
        for architecture in architectures {
            if !is_known(&architecture) {
                ctx.diagnostic(Diagnostic::warn(architecture));
            }
        }
    }
}

fn is_known(architecture: &str) -> bool {
    architecture == "all"
        || KNOWN.names.contains(architecture)
        || KNOWN.wildcards.contains(architecture)
}

/// Lintian's `Lintian::Data::Architectures`, from the vendored host table.
///
/// `names`: each host name, and on Linux the alias (`linux-amd64` gives
/// `amd64`; `amd64` gives `linux-amd64`). `wildcards`: the keys lintian
/// builds from every host's ABI, libc, OS, and CPU, with `any` in place of
/// each part it can be.
fn known(json: &str) -> Known {
    let mut names = HashSet::new();
    let mut wildcards = HashSet::new();
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Known { names, wildcards };
    };
    let Some(hosts) = value
        .get("host_variables")
        .and_then(|hosts| hosts.as_object())
    else {
        return Known { names, wildcards };
    };
    for (hyphenated, variables) in hosts {
        let field = |name: &str| {
            variables
                .get(name)
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string()
        };
        let (abi, libc, os, cpu) = (
            field("DEB_HOST_ARCH_ABI"),
            field("DEB_HOST_ARCH_LIBC"),
            field("DEB_HOST_ARCH_OS"),
            field("DEB_HOST_ARCH_CPU"),
        );
        names.insert(hyphenated.clone());
        if os == "linux" {
            match hyphenated.split_once('-') {
                Some(("linux", short)) => names.insert(short.to_string()),
                _ => names.insert(format!("linux-{hyphenated}")),
            };
        }
        wildcards.extend([
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
    Known { names, wildcards }
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
            // The Linux alias of a release architecture.
            arch("linux-amd64"),
            // Wildcards are not architectures, but they are known values here.
            arch("any"),
            arch("any-amd64"),
            arch("linux-any"),
            // A udeb with a known architecture.
            arch("amd64").udeb(),
        ];
        let fail = vec![
            arch("weird"),
            // `source` is only valid in `.changes` and `.buildinfo`.
            arch("source"),
            // One hint per unknown value, in order.
            arch("amd64 weird i386 other"),
            arch("linux-weird").udeb(),
        ];
        BinaryPackageTester::new(UnknownArchitecture, pass, fail).test_and_snapshot();
    }
}
