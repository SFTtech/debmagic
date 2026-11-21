use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::LazyLock;

use serde::Deserialize;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct MultiArchSamePackageHasArchSpecificOverrides;

declare_rule! {
    /// A `Multi-Arch: same` binary package has architecture-specific lintian overrides.
    ///
    /// For each installable with `Multi-Arch: same`, the Rule expands its
    /// `Architecture` into the release architectures it matches, then appends
    /// the wildcard itself. Each port then gives the path
    /// `debian/<package>.lintian-overrides.<port>`, which is reported when
    /// the packaging tree has it (a file or a directory; a symlink counts as
    /// present). Extra is empty; the path is the Location, compared as a
    /// hint pointer.
    ///
    /// Differences from lintian: lintian asks `dpkg-architecture` for the
    /// wildcard matches, and this Rule matches them against the vendored
    /// `data/architectures/host.json` (the names and wildcard forms
    /// `dpkg-architecture` accepts). Ports are checked in name order, not
    /// dpkg's, so several hits come out in a different order. A dangling
    /// symlink counts as present, where lintian's `resolve_path` does not.
    MultiArchSamePackageHasArchSpecificOverrides,
    code = "LN1056",
    tag = "multi-arch-same-package-has-arch-specific-overrides",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MultiArchSamePackageHasArchSpecificOverrides {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let same: Vec<(String, String)> = control
            .installables()
            .iter()
            .filter(|installable| {
                installable.fields().unfolded_value("Multi-Arch").as_deref() == Some("same")
            })
            .map(|installable| {
                (
                    installable.name().to_string(),
                    installable
                        .fields()
                        .unfolded_value("Architecture")
                        .unwrap_or_default(),
                )
            })
            .collect();
        if same.is_empty() {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        let mut hits = Vec::new();
        for (name, wildcard) in &same {
            for port in ports(wildcard) {
                let path = format!("debian/{name}.lintian-overrides.{port}");
                if files.get(&path).is_some() {
                    hits.push(path);
                }
            }
        }
        for path in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: PathBuf::from(path),
                line: None,
                column: None,
            }));
        }
    }
}

/// The architectures a wildcard matches, then the wildcard itself, without repeats.
fn ports(wildcard: &str) -> Vec<String> {
    let mut ports: Vec<String> = KNOWN
        .iter()
        .filter(|(name, tuple)| {
            name.as_str() == wildcard || (has_any(wildcard) && matches_wildcard(tuple, wildcard))
        })
        .map(|(name, _)| name.clone())
        .collect();
    ports.push(wildcard.to_string());
    let mut seen = std::collections::HashSet::new();
    ports.retain(|port| seen.insert(port.clone()));
    ports
}

fn has_any(wildcard: &str) -> bool {
    wildcard.split('-').any(|part| part == "any")
}

/// dpkg's wildcard matching: the wildcard's parts line up with the right-most
/// fields of `abi-libc-os-cpu`, and each part is `any` or the same value.
fn matches_wildcard(tuple: &[String; 4], wildcard: &str) -> bool {
    let parts: Vec<&str> = wildcard.split('-').collect();
    if parts.len() > tuple.len() {
        return false;
    }
    let offset = tuple.len() - parts.len();
    parts
        .iter()
        .enumerate()
        .all(|(index, part)| *part == "any" || *part == tuple[offset + index])
}

#[derive(Deserialize)]
struct HostFile {
    host_variables: BTreeMap<String, HostArch>,
}

#[derive(Deserialize)]
struct HostArch {
    #[serde(rename = "DEB_HOST_ARCH_ABI")]
    abi: String,
    #[serde(rename = "DEB_HOST_ARCH_LIBC")]
    libc: String,
    #[serde(rename = "DEB_HOST_ARCH_OS")]
    os: String,
    #[serde(rename = "DEB_HOST_ARCH_CPU")]
    cpu: String,
}

/// Each known Debian architecture name with its `abi-libc-os-cpu` tuple.
static KNOWN: LazyLock<Vec<(String, [String; 4])>> = LazyLock::new(|| {
    let file: HostFile =
        serde_json::from_str(include_str!("../../../data/architectures/host.json"))
            .expect("vendored architectures/host.json");
    let mut known = BTreeMap::new();
    for (hyphenated, arch) in file.host_variables {
        let tuple = [arch.abi, arch.libc, arch.os.clone(), arch.cpu];
        if arch.os == "linux" {
            let short = match hyphenated.strip_prefix("linux-") {
                Some(short) => short.to_string(),
                None => format!("linux-{hyphenated}"),
            };
            known.insert(short, tuple.clone());
        }
        known.insert(hyphenated, tuple);
    }
    known.into_iter().collect()
});

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
\n\
Package: example\n\
Architecture: any\n\
Multi-Arch: same\n\
Description: example\n extra\n";

    /// A packaging tree with `CONTROL` replaced by one binary paragraph and
    /// the given overrides files, each a `debian/...lintian-overrides.<port>`.
    fn package(control: &str, overrides: &[&str]) -> Vec<(String, TestFile)> {
        let mut files = vec![
            ("debian/control".to_string(), TestFile::new(control)),
            (
                "debian/rules".to_string(),
                TestFile::new("#!/usr/bin/make -f\n"),
            ),
        ];
        for path in overrides {
            files.push((path.to_string(), TestFile::new("foo\n")));
        }
        files
    }

    fn control(architecture: &str, multi: &str, name: &str) -> String {
        format!(
            "Source: example\n\
             Maintainer: Example <ex@example.com>\n\
             Standards-Version: 4.7.2\n\
             \n\
             Package: {name}\n\
             Architecture: {architecture}\n\
             Multi-Arch: {multi}\n\
             Description: example\n extra\n"
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            package(CONTROL, &[]),
            package(CONTROL, &["debian/example.lintian-overrides"]),
            package(
                &control("all", "same", "example"),
                &["debian/example.lintian-overrides.amd64"],
            ),
            package(
                &control("any", "foreign", "example"),
                &["debian/example.lintian-overrides.amd64"],
            ),
            package(
                &control("any", "same", "example"),
                &["debian/example-nonrel.lintian-overrides.amd64"],
            ),
            package(CONTROL, &["debian/other.lintian-overrides.amd64"]),
        ];
        let fail = vec![
            package(CONTROL, &["debian/example.lintian-overrides.amd64"]),
            package(
                &control("amd64", "same", "example"),
                &["debian/example.lintian-overrides.amd64"],
            ),
            package(
                &control("linux-any", "same", "example"),
                &["debian/example.lintian-overrides.amd64"],
            ),
            package(
                &control("any", "same", "example"),
                &[
                    "debian/example.lintian-overrides.amd64",
                    "debian/example.lintian-overrides.arm64",
                ],
            ),
        ];
        SourceTreeTester::new(
            MultiArchSamePackageHasArchSpecificOverrides,
            pass.clone(),
            fail.clone(),
        )
        .test_and_snapshot();
        SourcePackageTester::new(
            MultiArchSamePackageHasArchSpecificOverrides,
            pass.into_iter()
                .map(SourcePackageCase::from)
                .collect::<Vec<_>>(),
            fail.into_iter()
                .map(SourcePackageCase::from)
                .collect::<Vec<_>>(),
        )
        .test_and_snapshot();
    }
}
