use std::path::{Component, PathBuf};

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::{Entry, EntryKind, FileTree};
use crate::lint::rule::SourceRule;

pub struct GoLibraryPackageDoesNotIncludeVersion;

declare_rule! {
    /// A Golang library package whose upstream module path ends in a major
    /// version suffix `/vN` does not carry `-vN-dev` in its name.
    ///
    /// Skipped unless the source paragraph's `Build-Depends` imply
    /// `golang-go | golang-any` and the source declares `XS-Go-Import-Path`
    /// (lintian reports the missing field under another tag). The module path
    /// is the first `module` line of `go.mod` in the packaging files
    /// (the Source tree, or the Patched files of a Source package); a
    /// symlinked `go.mod` is followed inside the tree. The `module` argument
    /// keeps a quote that wraps it, so a quoted path has no version suffix, as
    /// in lintian. For each installable named `golang-*-dev` that does not
    /// end in `-vN-dev` for that `N`, unless its Section (or else the source
    /// Section) is `oldlibs`, the installable name is reported. Extra is the
    /// installable name; Location is empty.
    ///
    /// Differs from lintian: a symlink is only followed at the final path
    /// component of `go.mod`, which sits at the tree root.
    GoLibraryPackageDoesNotIncludeVersion,
    code = "LN1104",
    tag = "go-library-package-does-not-include-version",
    default_selected = true,
    experimental = false,
}

/// How many symlinks [`resolve_regular`] follows before giving up.
const MAX_SYMLINK_HOPS: usize = 8;

impl SourceRule for GoLibraryPackageDoesNotIncludeVersion {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let source = control.source();
        let builds_with_go = source
            .relation("Build-Depends")
            .is_some_and(|depends| depends.satisfies("golang-go | golang-any"));
        if !builds_with_go || !source.declares("XS-Go-Import-Path") {
            return;
        }
        let source_section = trimmed_non_empty(source.get("Section"));
        let installables: Vec<(String, Option<String>)> = control
            .installables()
            .iter()
            .map(|installable| {
                let section = trimmed_non_empty(installable.fields().get("Section"))
                    .or_else(|| source_section.clone());
                (installable.name().to_owned(), section)
            })
            .collect();

        let Some(files) = ctx.files() else {
            return;
        };
        let Some(entry) = resolve_regular(files, "go.mod") else {
            return;
        };
        let Ok(go_mod) = files.read(entry) else {
            return;
        };
        let Some(version) = major_version(&go_mod) else {
            return;
        };

        for (name, section) in installables {
            if is_unversioned_dev(&name, &version) && section.as_deref() != Some("oldlibs") {
                ctx.diagnostic(Diagnostic::warn(name));
            }
        }
    }
}

/// The value with surrounding space trimmed, `None` when absent or empty.
fn trimmed_non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// The `N` of a module path ending in `/vN`, from the first `module` line.
fn major_version(go_mod: &[u8]) -> Option<String> {
    let path = go_mod
        .split(|byte| *byte == b'\n')
        .find_map(module_argument)?;
    let digits = path
        .iter()
        .rev()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    let (prefix, digits) = path.split_at(path.len() - digits);
    if digits.is_empty() || !prefix.ends_with(b"/v") {
        return None;
    }
    Some(String::from_utf8_lossy(digits).into_owned())
}

/// The argument of a `module` directive on one line, as lintian's
/// `^ \s* module \s+ "? (\S+) "?` reads it. The opening quote is taken
/// off only when a non-space byte follows it.
fn module_argument(line: &[u8]) -> Option<&[u8]> {
    let rest = line.trim_ascii_start().strip_prefix(b"module")?;
    let after_space = rest.trim_ascii_start();
    if after_space.len() == rest.len() {
        return None;
    }
    let argument = match after_space {
        [b'"', next, ..] if !next.is_ascii_whitespace() => &after_space[1..],
        _ => after_space,
    };
    let end = argument
        .iter()
        .position(u8::is_ascii_whitespace)
        .unwrap_or(argument.len());
    (end > 0).then(|| &argument[..end])
}

/// `golang-*-dev` (at least one byte between) that does not end in `-v<major>-dev`.
fn is_unversioned_dev(name: &str, major: &str) -> bool {
    let Some(middle) = name
        .strip_prefix("golang-")
        .and_then(|rest| rest.strip_suffix("-dev"))
    else {
        return false;
    };
    if middle.is_empty() || middle.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return false;
    }
    let versioned_suffix = format!("-v{major}");
    !(middle.len() > versioned_suffix.len() && middle.ends_with(&versioned_suffix))
}

/// The regular file or hardlink at `path`, following symlinks inside the tree.
fn resolve_regular<'t>(tree: &'t FileTree, path: &str) -> Option<&'t Entry> {
    let mut current = tree.get(path)?;
    for _ in 0..MAX_SYMLINK_HOPS {
        match current.kind() {
            EntryKind::RegularFile | EntryKind::Hardlink => return Some(current),
            EntryKind::Symlink => current = tree.get(&symlink_target(current)?)?,
            EntryKind::Directory
            | EntryKind::Fifo
            | EntryKind::CharDevice
            | EntryKind::BlockDevice => return None,
        }
    }
    None
}

/// Where a symlink points, relative to the tree root.
fn symlink_target(link: &Entry) -> Option<PathBuf> {
    let mut resolved = link.path().parent()?.to_path_buf();
    for component in link.link_target()?.components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !resolved.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(resolved)
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const GO_SOURCE: &str = "Build-Depends: debhelper-compat (= 14), golang-go (>= 2:1.22)\nXS-Go-Import-Path: example.org/x/v2\n";

    fn package(name: &str, section: Option<&str>) -> String {
        let section = section
            .map(|s| format!("Section: {s}\n"))
            .unwrap_or_default();
        format!("Package: {name}\nArchitecture: all\n{section}Description: example\n extra\n\n")
    }

    /// A Source tree with `debian/control` (source paragraph and installables) and `go.mod`.
    fn case(source: &str, installables: &str, go_mod: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            (
                "debian/control",
                TestFile::new(format!("Source: example\n{source}\n{installables}")),
            ),
            ("go.mod", TestFile::new(go_mod)),
        ]
    }

    const V2: &str = "module example.org/x/v2\n\ngo 1.27\n";

    #[test]
    fn test() {
        let dev = package("golang-x-dev", None);
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            // Not a Golang build.
            case(
                "Build-Depends: debhelper-compat (= 14)\nXS-Go-Import-Path: example.org/x/v2\n",
                &dev,
                V2,
            ),
            // XS-Go-Import-Path missing: lintian reports that under another tag.
            case("Build-Depends: golang-go\n", &dev, V2),
            // No version suffix on the module path.
            case(GO_SOURCE, &dev, "module example.org/x\n"),
            // The module path quote is part of the argument: no version suffix.
            case(GO_SOURCE, &dev, "module \"example.org/x/v2\"\n"),
            // The installable carries the version suffix.
            case(GO_SOURCE, &package("golang-x-v2-dev", None), V2),
            // Not a library: name does not end in -dev.
            case(GO_SOURCE, &package("golang-x-tools", None), V2),
            // oldlibs installable, and oldlibs source Section stands in for a missing one.
            case(GO_SOURCE, &package("golang-x-dev", Some("oldlibs")), V2),
            case(&format!("Section: oldlibs\n{GO_SOURCE}"), &dev, V2),
            // No go.mod, and a go.mod without a module line.
            vec![(
                "debian/control",
                TestFile::new(format!("Source: example\n{GO_SOURCE}\n{dev}")),
            )],
            case(GO_SOURCE, &dev, "// nothing\ngo 1.27\n"),
        ];
        let fail = vec![
            case(GO_SOURCE, &dev, V2),
            // Only the unversioned one is reported.
            case(
                GO_SOURCE,
                &format!("{dev}{}", package("golang-x-v2-dev", None)),
                V2,
            ),
            // A different major version than the module path.
            case(GO_SOURCE, &package("golang-x-v3-dev", None), V2),
            // A non-oldlibs Section on the installable wins over an oldlibs source.
            case(
                &format!("Section: oldlibs\n{GO_SOURCE}"),
                &package("golang-x-dev", Some("libs")),
                V2,
            ),
            // A comment line before the module line; a non-UTF-8 go.mod.
            case(GO_SOURCE, &dev, "// comment\nmodule example.org/x/v2\n"),
            vec![
                (
                    "debian/control",
                    TestFile::new(format!("Source: example\n{GO_SOURCE}\n{dev}")),
                ),
                (
                    "go.mod",
                    TestFile::from_bytes(b"module example.org/x/v2\n// \xff\n".to_vec()),
                ),
            ],
            // A symlinked go.mod is followed inside the tree.
            vec![
                (
                    "debian/control",
                    TestFile::new(format!("Source: example\n{GO_SOURCE}\n{dev}")),
                ),
                ("go.mod", TestFile::symlink("real.mod")),
                ("real.mod", TestFile::new(V2)),
            ],
        ];
        SourceTreeTester::new(GoLibraryPackageDoesNotIncludeVersion, pass, fail)
            .test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let dsc = Dsc::parse("Source: example\n").expect("Dsc");
        let dev = package("golang-x-dev", None);
        let pass = vec![
            SourcePackageCase::from(case(GO_SOURCE, &dev, "module example.org/x\n"))
                .dsc(dsc.clone()),
        ];
        let fail = vec![SourcePackageCase::from(case(GO_SOURCE, &dev, V2)).dsc(dsc)];
        SourcePackageTester::new(GoLibraryPackageDoesNotIncludeVersion, pass, fail)
            .test_and_snapshot();
    }
}
