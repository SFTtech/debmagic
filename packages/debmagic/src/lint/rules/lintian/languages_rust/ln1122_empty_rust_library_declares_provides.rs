use std::os::unix::ffi::OsStrExt;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::FileTree;
use crate::lint::rule::BinaryPackageRule;

pub struct EmptyRustLibraryDeclaresProvides;

declare_rule! {
    /// A `librust-*` Binary package declares a non-empty `Provides` field but
    /// ships nothing outside its own documentation.
    ///
    /// Skipped unless the package name starts with `librust-` and the
    /// Installation control has a non-empty `Provides`. Emptiness follows
    /// lintian's `not_just_docs` (which is true when the package is empty):
    /// every Data files entry other than a directory must be under
    /// `usr/share/doc/<package>` (a prefix match, as lintian's regex), or be
    /// the package's own lintian override or reportbug file. A package with no
    /// Data files counts as empty. udebs are not skipped. Extra and Location
    /// are empty.
    EmptyRustLibraryDeclaresProvides,
    code = "LN1122",
    tag = "empty-rust-library-declares-provides",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for EmptyRustLibraryDeclaresProvides {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        if !package.starts_with("librust-") {
            return;
        }
        let declares_provides = ctx
            .installation_control()
            .fields()
            .get("Provides")
            .is_some_and(|value| !value.trim().is_empty());
        if !declares_provides {
            return;
        }
        let empty = ctx
            .data_files()
            .is_none_or(|files| is_empty(files, &package));
        if empty {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

/// lintian's `not_just_docs` for a Binary package: `true` when no Data files
/// entry, other than a directory, lies outside the package's documentation
/// and its own override and reportbug files.
fn is_empty(files: &FileTree, package: &str) -> bool {
    files
        .entries()
        .iter()
        .filter(|entry| !entry.is_dir())
        .all(|entry| is_docs_or_package_metadata(entry.path().as_os_str().as_bytes(), package))
}

fn is_docs_or_package_metadata(path: &[u8], package: &str) -> bool {
    let doc_dir = format!("usr/share/doc/{package}");
    if path.starts_with(doc_dir.as_bytes()) {
        return true;
    }
    let overrides = format!("usr/share/lintian/overrides/{package}");
    let bug = format!("usr/share/bug/{package}");
    [
        overrides.clone(),
        format!("{overrides}.gz"),
        bug.clone(),
        format!("{bug}/control"),
        format!("{bug}/presubj"),
        format!("{bug}/script"),
    ]
    .iter()
    .any(|name| path == name.as_bytes())
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn with_provides(package: &str, files: Vec<(&str, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(files)
            .package(package)
            .field("Provides", "librust-foo-dev (= 1.0)")
    }

    fn docs() -> (&'static str, TestFile) {
        (
            "usr/share/doc/librust-foo-dev/changelog.gz",
            TestFile::new(""),
        )
    }

    #[test]
    fn binary() {
        let pass = vec![
            // Not a librust package.
            with_provides("libfoo-dev", vec![docs()]),
            // No Provides field, or an empty one.
            BinaryPackageCase::from(vec![docs()]).package("librust-foo-dev"),
            BinaryPackageCase::from(vec![docs()])
                .package("librust-foo-dev")
                .field("Provides", ""),
            // Ships code: not empty.
            with_provides(
                "librust-foo-dev",
                vec![("usr/lib/librust-foo-dev/libfoo.rlib", TestFile::new(""))],
            ),
            // A symlink outside the documentation is not a directory.
            with_provides(
                "librust-foo-dev",
                vec![("usr/lib/librust-foo-dev/link", TestFile::symlink("x"))],
            ),
        ];
        let fail = vec![
            // Documentation only.
            with_provides("librust-foo-dev", vec![docs()]),
            // A directory outside the documentation is not a file: still empty.
            with_provides(
                "librust-foo-dev",
                vec![docs(), ("usr/lib/librust-foo-dev", TestFile::directory())],
            ),
            // Lintian's own override and reportbug files do not count.
            with_provides(
                "librust-foo-dev",
                vec![
                    (
                        "usr/share/lintian/overrides/librust-foo-dev",
                        TestFile::new(""),
                    ),
                    ("usr/share/bug/librust-foo-dev/script", TestFile::new("")),
                ],
            ),
            // A udeb is not skipped.
            with_provides("librust-foo-dev", vec![docs()]).udeb(),
            // No Data files at all.
            with_provides("librust-foo-dev", vec![]),
        ];
        BinaryPackageTester::new(EmptyRustLibraryDeclaresProvides, pass, fail).test_and_snapshot();
    }
}
