use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

pub struct EmptyBinaryPackage;

declare_rule! {
    /// The Binary package ships nothing beyond its standard documentation.
    ///
    /// Walks Data files entries other than directories. Only files directly in
    /// `usr/share/doc/<pkg>` that are standard (changelog, copyright, NEWS, any
    /// `README*`, ...) or architecture bookkeeping (`changelog.Debian.<arch>.gz`,
    /// `buildinfo_<arch>.gz`), and lintian override / reportbug files for the
    /// package, leave it empty. Skipped for udebs, transitional packages, and
    /// metapackages (by Description, Section, or a `task-` name). Extra and
    /// Location are empty.
    EmptyBinaryPackage,
    code = "LN0007",
    tag = "empty-binary-package",
    default_selected = true,
    experimental = false,
}

/// lintian `files/standard-files`: not enough to make a package non-empty.
const STANDARD_FILES: &[&str] = &[
    "AUTHORS",
    "AUTHORS.gz",
    "BUGS",
    "BUGS.gz",
    "changelog.Debian.gz",
    "changelog.gz",
    "CHANGES",
    "CHANGES.gz",
    "COPYING.gz",
    "copyright",
    "HACKING",
    "HACKING.gz",
    "NEWS",
    "NEWS.Debian",
    "NEWS.Debian.gz",
    "NEWS.gz",
    "TODO",
    "TODO.gz",
];

static TRANSITIONAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)transitional package").expect("valid regex"));
static META_PACKAGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)meta[ -]?package|(?:dependency|dummy|empty) package").expect("valid regex")
});
static META_SECTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|/)(?:tasks|metapackages)$").expect("valid regex"));
static README: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^README(?:\..*)?$").expect("valid regex"));

impl BinaryPackageRule for EmptyBinaryPackage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let package = ctx.package_name().to_string();
        let fields = ctx.installation_control().fields().clone();
        if is_dummy(&package, &fields) {
            return;
        }
        let architecture = fields.get("Architecture").unwrap_or_default().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        if is_empty(files, &package, &architecture) {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

/// Transitional or meta package, by lintian's `is_transitional` / `is_meta_package`.
fn is_dummy(package: &str, fields: &ControlParagraph) -> bool {
    let description = fields.get("Description").unwrap_or_default();
    let section = fields.get("Section").unwrap_or_default();
    TRANSITIONAL.is_match(description)
        || META_PACKAGE.is_match(description)
        || META_SECTION.is_match(section)
        || package.starts_with("task-")
}

fn is_empty(files: &FileTree, package: &str, architecture: &str) -> bool {
    !files
        .query()
        .kinds([
            EntryKind::RegularFile,
            EntryKind::Hardlink,
            EntryKind::Symlink,
        ])
        .into_iter()
        .any(|entry| {
            entry
                .path()
                .to_str()
                .is_none_or(|path| makes_non_empty(path, package, architecture))
        })
}

fn makes_non_empty(path: &str, package: &str, architecture: &str) -> bool {
    let doc_dir = format!("usr/share/doc/{package}");
    if !path.starts_with(&doc_dir) {
        return !is_package_override_or_bug_file(path, package);
    }
    // Anything in a subdirectory of the package's doc directory.
    if let Some(rest) = path
        .strip_prefix(&doc_dir)
        .and_then(|rest| rest.strip_prefix('/'))
        && rest
            .split_once('/')
            .is_some_and(|(name, _)| !name.is_empty())
    {
        return true;
    }
    if path == doc_dir {
        return false;
    }
    let basename = path.rsplit('/').next().unwrap_or(path);
    if STANDARD_FILES.contains(&basename) || README.is_match(basename) {
        return false;
    }
    if architecture != "all" && basename == format!("changelog.Debian.{architecture}.gz") {
        return false;
    }
    basename != format!("buildinfo_{architecture}.gz")
}

/// Lintian overrides and reportbug scripts of the package itself.
fn is_package_override_or_bug_file(path: &str, package: &str) -> bool {
    let overrides = format!("usr/share/lintian/overrides/{package}");
    let bug = format!("usr/share/bug/{package}");
    path == overrides
        || path == format!("{overrides}.gz")
        || path == bug
        || ["control", "presubj", "script"]
            .iter()
            .any(|name| path == format!("{bug}/{name}"))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn files(paths: &[&str]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            paths
                .iter()
                .map(|path| (path.to_string(), TestFile::new("content")))
                .collect::<Vec<_>>(),
        )
    }

    fn docs_only() -> BinaryPackageCase {
        docs_of("example")
    }

    fn docs_of(package: &str) -> BinaryPackageCase {
        files(&[
            &format!("usr/share/doc/{package}/copyright"),
            &format!("usr/share/doc/{package}/changelog.Debian.gz"),
        ])
        .package(package)
    }

    #[test]
    fn test() {
        let pass = vec![
            files(&["usr/bin/example"]),
            files(&[
                "usr/share/doc/example/copyright",
                "usr/share/doc/example/examples/demo",
            ]),
            files(&[
                "usr/share/doc/example/copyright",
                "usr/share/doc/example/FAQ",
            ]),
            // A doc path that only shares the package name as a prefix.
            files(&["usr/share/doc/example-data"]),
            files(&["usr/share/doc/example-common/NOTES"]),
            // A symlink outside the doc directory is content.
            BinaryPackageCase::from(vec![("usr/bin/e", TestFile::symlink("../lib/e"))]),
            // Another package's override does not count.
            files(&["usr/share/lintian/overrides/other"]),
            docs_only().field("Description", "A metapackage for examples\n more"),
            docs_only().field("Description", "dummy package"),
            docs_only().field("Description", "Dependency Package"),
            docs_only().field("Description", "empty package"),
            docs_only().field("Description", "Transitional package for foo"),
            docs_only().field("Description", "meta-package"),
            docs_only().field("Section", "metapackages"),
            docs_only().field("Section", "universe/tasks"),
            docs_of("task-example"),
            docs_only().udeb(),
        ];
        let fail = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![("usr", TestFile::directory())]),
            docs_only(),
            files(&["usr/share/doc/example/README"]),
            files(&["usr/share/doc/example/README.Debian"]),
            files(&["usr/share/doc/example/readme.txt"]),
            files(&["usr/share/doc/example/NEWS.Debian.gz"]),
            // Lintian matches the doc directory by prefix, so a sibling directory's
            // standard files are ignored too.
            files(&["usr/share/doc/example-common/copyright"]),
            // The doc directory may be a symlink.
            BinaryPackageCase::from(vec![("usr/share/doc/example", TestFile::symlink("other"))]),
            files(&[
                "usr/share/lintian/overrides/example",
                "usr/share/bug/example/presubj",
            ]),
            files(&["usr/share/lintian/overrides/example.gz"]),
            files(&["usr/share/bug/example"]),
            files(&["usr/share/doc/example/changelog.Debian.amd64.gz"])
                .field("Architecture", "amd64"),
            files(&["usr/share/doc/example/buildinfo_amd64.gz"]).field("Architecture", "amd64"),
            // A metapackage-looking Description elsewhere than a phrase.
            docs_only().field("Description", "an example package"),
            docs_only().field("Section", "tasks-extra"),
            docs_of("tasks-example"),
        ];
        BinaryPackageTester::new(EmptyBinaryPackage, pass, fail).test_and_snapshot();
    }

    #[test]
    fn mismatched_arch_bookkeeping_is_content() {
        let pass = vec![
            files(&["usr/share/doc/example/changelog.Debian.amd64.gz"])
                .field("Architecture", "all"),
            files(&["usr/share/doc/example/buildinfo_amd64.gz"]).field("Architecture", "all"),
            files(&["usr/share/doc/example/changelog.Debian.arm64.gz"])
                .field("Architecture", "amd64"),
        ];
        BinaryPackageTester::new(EmptyBinaryPackage, pass, Vec::<BinaryPackageCase>::new()).test();
    }
}
