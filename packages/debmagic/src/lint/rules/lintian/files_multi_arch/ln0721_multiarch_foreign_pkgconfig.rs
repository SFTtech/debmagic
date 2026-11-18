use std::collections::HashMap;
use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct MultiarchForeignPkgconfig;

declare_rule! {
    /// The Binary package is `Multi-Arch: foreign` but ships a pkg-config file
    /// in its architecture's public pkg-config directory.
    ///
    /// Only when Multi-Arch is exactly `foreign` and Architecture has a
    /// DEB_HOST_MULTIARCH triplet (`all` has none). Walks the Data files that
    /// are not directories (symlinks and hardlinks included) and reports each
    /// `usr/lib/<triplet>/pkgconfig/<name>.pc` at its path, with an empty
    /// extra. Udebs are checked too.
    MultiarchForeignPkgconfig,
    code = "LN0721",
    tag = "multiarch-foreign-pkgconfig",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MultiarchForeignPkgconfig {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let fields = ctx.installation_control().fields();
        if fields.get("Multi-Arch") != Some("foreign") {
            return;
        }
        let Some(triplet) = fields
            .get("Architecture")
            .and_then(|architecture| HOST_MULTIARCH.get(architecture))
            .cloned()
        else {
            return;
        };
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<_> = files
            .entries()
            .iter()
            .filter(|entry| entry.kind() != EntryKind::Directory)
            .filter(|entry| {
                entry
                    .path()
                    .to_str()
                    .is_some_and(|name| is_public_pkgconfig(name, &triplet))
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in hits {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// `usr/lib/<triplet>/pkgconfig/<name>.pc`, with `<name>` a single component.
fn is_public_pkgconfig(name: &str, triplet: &str) -> bool {
    let Some(tail) = name
        .strip_prefix("usr/lib/")
        .and_then(|rest| rest.strip_prefix(triplet))
        .and_then(|rest| rest.strip_prefix('/'))
    else {
        return false;
    };
    let Some(file) = tail.strip_prefix("pkgconfig/") else {
        return false;
    };
    file.len() > ".pc".len() && file.ends_with(".pc") && !file.contains('/')
}

/// `DEB_HOST_ARCH` to `DEB_HOST_MULTIARCH`, from the vendored `host.json`.
static HOST_MULTIARCH: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    let file: serde_json::Value =
        serde_json::from_str(include_str!("../../../data/architectures/host.json"))
            .expect("vendored architectures/host.json");
    file["host_variables"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(_, host)| {
            Some((
                host["DEB_HOST_ARCH"].as_str()?.to_string(),
                host["DEB_HOST_MULTIARCH"].as_str()?.to_string(),
            ))
        })
        .collect()
});

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn pc() -> TestFile {
        TestFile::new("Name: foo\n")
    }

    fn foreign_amd64(files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files)
            .field("Architecture", "amd64")
            .field("Multi-Arch", "foreign")
    }

    #[test]
    fn public_pkgconfig_paths() {
        let check = |name: &str| is_public_pkgconfig(name, "x86_64-linux-gnu");
        assert!(check("usr/lib/x86_64-linux-gnu/pkgconfig/libfoo.pc"));
        assert!(check("usr/lib/x86_64-linux-gnu/pkgconfig/x.pc"));
        assert!(!check("usr/lib/x86_64-linux-gnu/pkgconfig/.pc"));
        assert!(!check("usr/lib/x86_64-linux-gnu/pkgconfig/libfoo.pc.in"));
        assert!(!check("usr/lib/x86_64-linux-gnu/pkgconfig/sub/libfoo.pc"));
        assert!(!check("usr/lib/x86_64-linux-gnu/pkgconfig/"));
        assert!(!check("usr/lib/i386-linux-gnu/pkgconfig/libfoo.pc"));
        assert!(!check("usr/lib/x86_64-linux-gnu/libfoo.pc"));
        assert!(!check("usr/lib/pkgconfig/libfoo.pc"));
        assert!(!check("usr/lib/x86_64-linux-gnu2/pkgconfig/libfoo.pc"));
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(vec![("usr/lib/x86_64-linux-gnu/pkgconfig/libfoo.pc", pc())])
                .field("Architecture", "amd64"),
            BinaryPackageCase::from(vec![("usr/lib/x86_64-linux-gnu/pkgconfig/libfoo.pc", pc())])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "same"),
            // Architecture all has no DEB_HOST_MULTIARCH triplet.
            BinaryPackageCase::from(vec![("usr/lib/x86_64-linux-gnu/pkgconfig/libfoo.pc", pc())])
                .field("Architecture", "all")
                .field("Multi-Arch", "foreign"),
            foreign_amd64(vec![("usr/lib/pkgconfig/libfoo.pc", pc())]),
            foreign_amd64(vec![("usr/lib/i386-linux-gnu/pkgconfig/libfoo.pc", pc())]),
            foreign_amd64(vec![(
                "usr/lib/x86_64-linux-gnu/pkgconfig/sub/libfoo.pc",
                pc(),
            )]),
            foreign_amd64(vec![(
                "usr/lib/x86_64-linux-gnu/pkgconfig/libfoo.pc.in",
                pc(),
            )]),
            foreign_amd64(vec![(
                "usr/lib/x86_64-linux-gnu/pkgconfig",
                TestFile::directory(),
            )]),
            foreign_amd64(vec![("usr/lib/x86_64-linux-gnu/libfoo.so", pc())]),
            // A `.pc` in a directory whose architecture has no triplet is not judged.
            BinaryPackageCase::from(vec![("usr/lib/foo/pkgconfig/libfoo.pc", pc())])
                .field("Architecture", "lintian-fake-arch")
                .field("Multi-Arch", "foreign"),
        ];
        let fail = vec![
            foreign_amd64(vec![("usr/lib/x86_64-linux-gnu/pkgconfig/libfoo.pc", pc())]),
            foreign_amd64(vec![
                ("usr/lib/x86_64-linux-gnu/pkgconfig/a.pc", pc()),
                ("usr/lib/x86_64-linux-gnu/pkgconfig/b.pc", pc()),
            ]),
            foreign_amd64(vec![(
                "usr/lib/x86_64-linux-gnu/pkgconfig/link.pc",
                TestFile::symlink("libfoo.pc"),
            )]),
            foreign_amd64(vec![(
                "usr/lib/x86_64-linux-gnu/pkgconfig/hard.pc",
                pc().hardlink(),
            )]),
            BinaryPackageCase::from(vec![("usr/lib/x86_64-linux-gnu/pkgconfig/libfoo.pc", pc())])
                .field("Architecture", "amd64")
                .field("Multi-Arch", "foreign")
                .udeb(),
        ];
        BinaryPackageTester::new(MultiarchForeignPkgconfig, pass, fail).test_and_snapshot();
    }
}
