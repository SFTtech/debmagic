use std::os::unix::ffi::OsStrExt;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::FileTree;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct PossibleDocumentationButNoDocBaseRegistration;

declare_rule! {
    /// A package ships HTML or PDF documentation under `usr/share/doc/` but
    /// has no `usr/share/doc-base/` folder.
    ///
    /// Reported as an info hint with no Location and no Extra. Packages named
    /// `libghc-*-doc` or `libghc6-*-doc` are exempt, as Haskell libraries
    /// register through the GHC documentation mechanism. A documentation file
    /// is a regular file or hardlink under `usr/share/doc/`, in at most one
    /// folder, whose name ends in `.html` or `.pdf` (an optional `.gz` is
    /// allowed). `changelog.html`, names starting with `README-` or `README.`,
    /// and names containing `examples` do not count. The doc-base folder is
    /// any entry at or under `usr/share/doc-base/`, so a symlinked folder is
    /// not followed here. Paths are compared as raw bytes.
    PossibleDocumentationButNoDocBaseRegistration,
    code = "LN1577",
    tag = "possible-documentation-but-no-doc-base-registration",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for PossibleDocumentationButNoDocBaseRegistration {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        if has_doc_base_folder(files) || !has_documentation(files) || is_haskell_doc(&package) {
            return;
        }
        ctx.diagnostic(Diagnostic::warn("").with_severity(Severity::Info));
    }
}

fn has_doc_base_folder(files: &FileTree) -> bool {
    files.entries().iter().any(|entry| {
        let path = entry.path().as_os_str().as_bytes();
        path == b"usr/share/doc-base" || path.starts_with(b"usr/share/doc-base/")
    })
}

fn has_documentation(files: &FileTree) -> bool {
    files
        .entries()
        .iter()
        .any(|entry| entry.is_file() && is_documentation(entry.path().as_os_str().as_bytes()))
}

/// lintian's `^libghc6?-.*-doc$`.
fn is_haskell_doc(package: &str) -> bool {
    let rest = package
        .strip_prefix("libghc-")
        .or_else(|| package.strip_prefix("libghc6-"));
    rest.is_some_and(|rest| rest.ends_with("-doc"))
}

/// `usr/share/doc/(?:[^/]+/)?(.+\.(?:html|pdf))(?:\.gz)?` with lintian's
/// exclusions on the captured name. The optional folder is tried first, as
/// the group is greedy, and the whole rest of the path otherwise.
fn is_documentation(path: &[u8]) -> bool {
    let Some(rest) = path.strip_prefix(b"usr/share/doc/") else {
        return false;
    };
    let with_folder = rest
        .iter()
        .position(|&byte| byte == b'/')
        .filter(|&slash| slash > 0)
        .and_then(|slash| name_of(&rest[slash + 1..]));
    let Some(name) = with_folder.or_else(|| name_of(rest)) else {
        return false;
    };
    !(name == b"changelog.html"
        || name.starts_with(b"README-")
        || name.starts_with(b"README.")
        || name
            .windows(b"examples".len())
            .any(|window| window == b"examples"))
}

/// The captured name of `.+\.(?:html|pdf)(?:\.gz)?` at the end of `text`.
fn name_of(text: &[u8]) -> Option<&[u8]> {
    let without_gz = text.strip_suffix(b".gz");
    match without_gz.filter(|core| has_doc_suffix(core)) {
        Some(core) => Some(core),
        None => has_doc_suffix(text).then_some(text),
    }
}

fn has_doc_suffix(text: &[u8]) -> bool {
    [b".html".as_slice(), b".pdf".as_slice()]
        .iter()
        .any(|suffix| text.len() > suffix.len() && text.ends_with(suffix))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // A doc-base folder is present.
            BinaryPackageCase::from(vec![
                ("usr/share/doc/example/manual.html", TestFile::new("x")),
                ("usr/share/doc-base/example", TestFile::new("x")),
            ]),
            // Only excluded names.
            BinaryPackageCase::from(vec![
                ("usr/share/doc/example/changelog.html", TestFile::new("x")),
                ("usr/share/doc/example/README.html", TestFile::new("x")),
                ("usr/share/doc/example/README-x.pdf", TestFile::new("x")),
                ("usr/share/doc/example/examples/x.html", TestFile::new("x")),
                ("usr/share/doc/example/index.txt", TestFile::new("x")),
            ]),
            // A symlink is not a documentation file.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/manual.html",
                TestFile::symlink("/elsewhere/manual.html"),
            )]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/manual.html",
                TestFile::new("x"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/manual.pdf.gz",
                TestFile::new("x"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/sub/x.pdf",
                TestFile::new("x"),
            )]),
            BinaryPackageCase::from(vec![("usr/share/doc/x.html", TestFile::new("x"))]).udeb(),
        ];
        BinaryPackageTester::new(PossibleDocumentationButNoDocBaseRegistration, pass, fail)
            .test_and_snapshot();
    }
}
