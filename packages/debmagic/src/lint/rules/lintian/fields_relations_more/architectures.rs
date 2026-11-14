//! Release architectures and architecture wildcards from lintian's
//! `data/architectures/host.json`, the same derivation as
//! `Lintian::Data::Architectures`.

use std::collections::HashSet;
use std::sync::LazyLock;

use serde::Deserialize;

#[derive(Deserialize)]
struct HostFile {
    host_variables: std::collections::HashMap<String, HostArch>,
}

#[derive(Deserialize)]
struct HostArch {
    #[serde(rename = "DEB_HOST_ARCH_ABI")]
    abi: String,
    #[serde(rename = "DEB_HOST_ARCH_CPU")]
    cpu: String,
    #[serde(rename = "DEB_HOST_ARCH_LIBC")]
    libc: String,
    #[serde(rename = "DEB_HOST_ARCH_OS")]
    os: String,
}

struct Architectures {
    names: HashSet<String>,
    wildcards: HashSet<String>,
}

static ARCHITECTURES: LazyLock<Architectures> = LazyLock::new(|| {
    let file: HostFile =
        serde_json::from_str(include_str!("../../../data/architectures/host.json"))
            .expect("vendored architectures/host.json");
    let mut names = HashSet::new();
    let mut wildcards = HashSet::new();
    for (hyphenated, arch) in file.host_variables {
        names.insert(hyphenated.clone());
        if arch.os == "linux" {
            if let Some(short) = hyphenated.strip_prefix("linux-") {
                names.insert(short.to_string());
            } else {
                names.insert(format!("linux-{hyphenated}"));
            }
        }
        wildcards.extend(wildcard_names(&arch));
    }
    Architectures { names, wildcards }
});

/// `all`, or neither a release architecture nor a wildcard (`any` is a wildcard).
pub(super) fn invalid_architecture_string(architecture: &str) -> bool {
    architecture == "all"
        || (!ARCHITECTURES.names.contains(architecture)
            && !ARCHITECTURES.wildcards.contains(architecture))
}

fn wildcard_names(arch: &HostArch) -> [String; 26] {
    let HostArch { abi, cpu, libc, os } = arch;
    [
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
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_architectures_and_wildcards() {
        assert!(!invalid_architecture_string("amd64"));
        assert!(!invalid_architecture_string("linux-amd64"));
        assert!(!invalid_architecture_string("alpha"));
        assert!(!invalid_architecture_string("sparc"));
        assert!(!invalid_architecture_string("any"));
        assert!(!invalid_architecture_string("linux-any"));
        assert!(!invalid_architecture_string("any-amd64"));
        assert!(!invalid_architecture_string("any-powerpc"));
        assert!(invalid_architecture_string("all"));
        assert!(invalid_architecture_string("source"));
        assert!(invalid_architecture_string("kfreebsd-i386"));
        assert!(invalid_architecture_string("kfreebsd-any"));
        assert!(invalid_architecture_string("lintian-fake-arch"));
        assert!(invalid_architecture_string("test"));
    }
}
