use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::DebianControl;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::package_relation::{Alternative, PackageRelation};
use crate::lint::rule::SourceRule;

pub struct AncientLibtool;

declare_rule! {
    /// A patched `ltconfig`, or an `ltmain.sh` older than 1.5.2-2, is shipped
    /// without regenerating libtool.
    ///
    /// Files under `debian/` are ignored. Suppressed when Build-Depends,
    /// Build-Depends-Indep, or Build-Depends-Arch implies
    /// `libtool | dh-autoreconf`. `ltconfig` has an empty extra. `ltmain.sh`
    /// extra is the `VERSION=` string. The Location is the file.
    AncientLibtool,
    code = "LN0387",
    tag = "ancient-libtool",
    default_selected = true,
    experimental = false,
}

const LIBTOOL: &[&str] = &["libtool", "dh-autoreconf"];
const ACCEPTABLE_MAJOR: u32 = 5;
const ACCEPTABLE_MINOR: u32 = 2;
const ACCEPTABLE_DEBIAN: u32 = 2;

static VERSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^VERSION=["']?(1\.(\d)\.(\d+)(?:-(\d))?)"#).expect("valid regex")
});

impl SourceRule for AncientLibtool {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if ctx.debian_control().is_some_and(build_depends_on_libtool) {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
            .into_iter()
            .filter(|entry| {
                let Some(path) = entry.path().to_str() else {
                    return false;
                };
                if path.starts_with("debian/") {
                    return false;
                }
                matches!(
                    entry.path().file_name().and_then(|name| name.to_str()),
                    Some("ltconfig" | "ltmain.sh")
                )
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        let mut hits = Vec::new();
        for path in paths {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if name == "ltconfig" {
                hits.push((path, String::new()));
                continue;
            }
            let Some(entry) = files.get(&path) else {
                continue;
            };
            let Ok(bytes) = files.read(entry) else {
                continue;
            };
            let text = String::from_utf8_lossy(&bytes);
            let Some(captures) = VERSION.captures(&text) else {
                continue;
            };
            let version = captures.get(1).map(|m| m.as_str()).unwrap_or("");
            let major: u32 = captures
                .get(2)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);
            let minor: u32 = captures
                .get(3)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);
            let debian: u32 = captures
                .get(4)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);
            let ancient = major < ACCEPTABLE_MAJOR
                || (major == ACCEPTABLE_MAJOR
                    && (minor < ACCEPTABLE_MINOR
                        || (minor == ACCEPTABLE_MINOR && debian < ACCEPTABLE_DEBIAN)));
            if ancient {
                hits.push((path, version.to_string()));
            }
        }
        for (path, version) in hits {
            ctx.diagnostic(Diagnostic::warn(version).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn build_depends_on_libtool(control: &DebianControl) -> bool {
    ["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"]
        .into_iter()
        .any(|field| {
            control
                .source()
                .relation(field)
                .is_some_and(|relation| implies(&relation, LIBTOOL))
        })
}

fn implies(relation: &PackageRelation, targets: &[&str]) -> bool {
    relation.dependencies().iter().any(|dependency| {
        if dependency.is_empty() {
            return false;
        }
        if dependency.len() == 1 {
            predicate_implies(&dependency[0], targets)
        } else {
            dependency
                .iter()
                .all(|alternative| predicate_implies(alternative, targets))
        }
    })
}

fn predicate_implies(alternative: &Alternative, targets: &[&str]) -> bool {
    if !alternative.rest().is_empty()
        || !alternative.architecture_qualifier().is_empty()
        || !alternative.architectures().is_empty()
        || !alternative.profiles().is_empty()
    {
        return false;
    }
    targets.contains(&alternative.package())
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(build_depends: &str) -> String {
        let depends = if build_depends.is_empty() {
            String::new()
        } else {
            format!("Build-Depends: {build_depends}\n")
        };
        format!(
            "Source: example\nMaintainer: Example <ex@example.com>\nStandards-Version: 4.7.2\n{depends}\nPackage: example\nArchitecture: all\nDescription: example\n extra\n"
        )
    }

    fn tree(build_depends: &str, extra: &[(&str, &str)]) -> Vec<(String, TestFile)> {
        let mut files = vec![(
            "debian/control".to_string(),
            TestFile::new(control(build_depends)),
        )];
        for (path, content) in extra {
            files.push(((*path).to_string(), TestFile::new(*content)));
        }
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            tree("", &[]),
            tree("", &[("debian/ltconfig", "")]),
            tree("", &[("ltmain.sh", "VERSION=\"1.5.2-2\"\n")]),
            tree("", &[("ltmain.sh", "VERSION=1.5.4\n")]),
            tree("libtool", &[("ltconfig", "")]),
            tree("dh-autoreconf", &[("ltmain.sh", "VERSION=1.3.5\n")]),
            tree("debhelper-compat (= 13), libtool", &[("ltconfig", "")]),
        ];
        let fail = vec![
            tree("", &[("ltconfig", "")]),
            tree("", &[("ltmain.sh", "# hello\nVERSION=\"1.3.5\"\n")]),
            tree("", &[("ltmain.sh", "VERSION=1.5.2\n")]),
            tree("", &[("src/ltmain.sh", "VERSION='1.5.2-1'\n")]),
        ];
        SourceTreeTester::new(AncientLibtool, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree("", &[]))];
        let fail = vec![SourcePackageCase::from(tree("", &[("ltconfig", "")]))];
        SourcePackageTester::new(AncientLibtool, pass, fail).test_and_snapshot();
    }
}
