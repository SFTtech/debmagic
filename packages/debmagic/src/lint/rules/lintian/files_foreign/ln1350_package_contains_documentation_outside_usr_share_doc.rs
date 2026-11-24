use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::FileTree;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct PackageContainsDocumentationOutsideUsrShareDoc;

declare_rule! {
    /// A documentation-looking file is installed outside `usr/share/doc`.
    ///
    /// Regular files and hardlinks whose basename looks like documentation
    /// (`.txt`, `.md`, `.pdf`, `README`, `COPYING`, `LICENSE`, `TODO`, ...,
    /// case-insensitively) are reported unless they are under `etc/`,
    /// `usr/share/doc/` or `usr/share/help/`, or one of lintian's exemptions
    /// applies: R site-library, SNMP MIBs, `template(s)` directories, a README
    /// whose text reads `thisdirectory` (lintian's `x` flag drops the space in
    /// "this directory", so the spaced text does not exempt it), ocaml and
    /// gtk-doc and GNUstep files, GNUstep Gorm `data.info`, and the Python
    /// Screen: a file in a `*.dist-info/` folder beside METADATA, or in a
    /// `*.egg-info/` folder beside PKG-INFO. Requirements files and
    /// `CMakeLists.txt` are
    /// not documentation. Udebs skip what they would report as
    /// `udeb-contains-documentation-file` (under `usr/share/doc` or
    /// `usr/share/info`). The entry's path is the Location; no extra. Reported
    /// at info severity.
    PackageContainsDocumentationOutsideUsrShareDoc,
    code = "LN1350",
    tag = "package-contains-documentation-outside-usr-share-doc",
    default_selected = false,
    experimental = false,
}

/// lintian's `$DOCUMENTATION_FILE_REGEX`, matched against the basename.
static DOCUMENTATION_FILE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:\.docx?$|\.html?$|\.info$|\.latex$|\.markdown$|\.md$|\.odt$|\.pdf$|\.readme$|\.rmd$|\.rst$|\.rtf$|\.tex$|\.txt$|^code[-_]of[-_]conduct$|^contribut(?:e|ing)$|^copyright$|^licen[sc]es?$|^howto$|^patents?$|^readme(?:[.]?first|[.]1st|[.]debian|[.]source)?$|^todos?$)",
    )
    .expect("valid regex")
});

/// lintian's `@NOT_DOCUMENTATION_FILE_REGEXES`, matched against the basename.
static NOT_DOCUMENTATION: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)^dependency_links[.]txt$",
        r"(?i)^requires[.]txt$",
        r"(?i)^requirements[.]txt$",
        r"(?i)^namespace_packages[.]txt$",
        r"(?i)^bindep[.]txt$",
        r"(?i)^version[.]txt$",
        r"(?i)^robots[.]txt$",
        r"(?i)^cmakelists[.]txt$",
    ]
    .iter()
    .map(|pattern| Regex::new(pattern).expect("valid regex"))
    .collect()
});

/// Lintian's `templates?(?:[.]d)?/` matched against the directory name.
static TEMPLATES_DIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"templates?(?:[.]d)?/").expect("valid regex"));

/// The README exemption: lintian's `$item->bytes =~ m{this directory}xi`. The
/// `x` flag ignores the space in the pattern, so the text must read
/// `thisdirectory` (any case) to exempt a README.
static THIS_DIRECTORY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)thisdirectory").expect("valid regex"));

/// lintian `Screen/Python/Egg/Metadata.pm`: a file in a `*.dist-info/` folder
/// beside a METADATA, or in a `*.egg-info/` folder beside a PKG-INFO, is masked.
/// `dirname` ends in `/` (or is empty at the top of the package).
fn masked_by_python_egg_screen(files: &FileTree, dirname: &str) -> bool {
    let Some(folder) = dirname.strip_suffix('/') else {
        return false;
    };
    let name = folder.rsplit('/').next().unwrap_or(folder);
    let beside = |sibling: &str| files.get(format!("{dirname}{sibling}")).is_some();
    // lintian's `[^/] [.] dist-info /`: a character before the suffix.
    let dist_info = name.len() > ".dist-info".len() && name.ends_with(".dist-info");
    let egg_info = name.len() > ".egg-info".len() && name.ends_with(".egg-info");
    (dist_info && beside("METADATA")) || (egg_info && beside("PKG-INFO"))
}

/// Udebs stop at these before the documentation test (lintian's early `return`).
static UDEB_DOC_OR_INFO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/share/(?:doc|info)/\S").expect("valid regex"));

impl BinaryPackageRule for PackageContainsDocumentationOutsideUsrShareDoc {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let is_udeb = ctx.is_udeb();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut reported: Vec<PathBuf> = Vec::new();
        for entry in files.entries() {
            if !entry.is_file() {
                continue;
            }
            let Some(name) = entry.path().to_str() else {
                continue;
            };
            if is_udeb && UDEB_DOC_OR_INFO.is_match(name) {
                continue;
            }
            let path = Path::new(name);
            let basename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            let dirname = path
                .parent()
                .and_then(|p| p.to_str())
                .map(|p| {
                    if p.is_empty() {
                        String::new()
                    } else {
                        format!("{p}/")
                    }
                })
                .unwrap_or_default();
            if !DOCUMENTATION_FILE.is_match(basename)
                || NOT_DOCUMENTATION.iter().any(|re| re.is_match(basename))
            {
                continue;
            }
            let exempt = name.starts_with("etc/")
                || name.starts_with("usr/share/doc/")
                || name.starts_with("usr/share/help/")
                || dirname.contains("usr/lib/R/site-library/")
                || dirname == "usr/share/snmp/mibs/"
                || TEMPLATES_DIR.is_match(&dirname)
                || (basename.to_ascii_lowercase().starts_with("readme")
                    && files.read(entry).is_ok_and(|bytes| {
                        THIS_DIRECTORY.is_match(&String::from_utf8_lossy(&bytes))
                    }))
                || name.starts_with("var/lib/ocaml/lintian/") && name.ends_with(".info")
                || name.starts_with("usr/share/gtk-doc/html/")
                    && (name.ends_with(".html") || name.ends_with(".htm"))
                || name.starts_with("usr/share/GNUstep/Documentation/")
                    && (name.ends_with(".html") || name.ends_with(".pdf"))
                || (dirname.contains(".gorm") && basename == "data.info")
                || masked_by_python_egg_screen(files, &dirname);
            if !exempt {
                reported.push(path.to_path_buf());
            }
        }
        for path in reported {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(path: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(content))]).field("Architecture", "all")
    }

    #[test]
    fn test() {
        let pass = vec![
            case("usr/share/doc/example/README.txt", "docs"),
            case("etc/example/NOTES.txt", "notes"),
            case("usr/share/example/requirements.txt", "dependency"),
            case("usr/share/example/README", "ThisDirectory is empty"),
            case("usr/share/example/template/LICENSE.md", "license"),
            case("usr/share/example/doc.pdf/inner", "not a doc"),
            // Udebs skip usr/share/info (the udeb early return).
            BinaryPackageCase::from(vec![("usr/share/info/example.txt", TestFile::new("info"))])
                .field("Architecture", "all")
                .udeb(),
        ];
        let fail = vec![
            case("usr/share/example/NOTES.txt", "notes"),
            case("usr/share/example/README", "real readme"),
            case("usr/share/man/man1/example.1.md", "text"),
            BinaryPackageCase::from(vec![(
                "usr/share/example/NOTES.txt",
                TestFile::new("notes"),
            )])
            .field("Architecture", "all")
            .udeb(),
        ];
        BinaryPackageTester::new(PackageContainsDocumentationOutsideUsrShareDoc, pass, fail)
            .test_and_snapshot();
    }

    fn folder(entries: &[(&str, &str)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            entries
                .iter()
                .map(|(path, content)| (path.to_string(), TestFile::new(*content)))
                .collect::<Vec<(String, TestFile)>>(),
        )
        .field("Architecture", "all")
    }

    /// lintian `Screen/Python/Egg/Metadata.pm` masks the hint for a file in a
    /// `*.dist-info/` folder that has a METADATA file beside it. The folder name
    /// needs at least one character before `.dist-info`.
    #[test]
    fn dist_info_documentation_is_masked_beside_metadata() {
        let pass = vec![folder(&[
            (
                "usr/lib/python3/dist-packages/foo-1.0.dist-info/METADATA",
                "m",
            ),
            (
                "usr/lib/python3/dist-packages/foo-1.0.dist-info/NOTES.txt",
                "n",
            ),
        ])];
        let fail = vec![
            // No METADATA beside the file.
            folder(&[(
                "usr/lib/python3/dist-packages/foo-1.0.dist-info/NOTES.txt",
                "n",
            )]),
            // A METADATA in a subfolder is not beside the file.
            folder(&[
                (
                    "usr/lib/python3/dist-packages/foo-1.0.dist-info/sub/METADATA",
                    "m",
                ),
                (
                    "usr/lib/python3/dist-packages/foo-1.0.dist-info/NOTES.txt",
                    "n",
                ),
            ]),
            // Only a folder whose name ends in .dist-info is masked.
            folder(&[
                (
                    "usr/lib/python3/dist-packages/foo-1.0.dist-info-old/METADATA",
                    "m",
                ),
                (
                    "usr/lib/python3/dist-packages/foo-1.0.dist-info-old/NOTES.txt",
                    "n",
                ),
            ]),
            // The folder name needs a character before .dist-info.
            folder(&[
                ("usr/lib/python3/dist-packages/.dist-info/METADATA", "m"),
                ("usr/lib/python3/dist-packages/.dist-info/NOTES.txt", "n"),
            ]),
            // An egg-info folder is not masked by METADATA.
            folder(&[
                ("usr/lib/python3/dist-packages/foo.egg-info/METADATA", "m"),
                ("usr/lib/python3/dist-packages/foo.egg-info/NOTES.txt", "n"),
            ]),
        ];
        BinaryPackageTester::new(PackageContainsDocumentationOutsideUsrShareDoc, pass, fail).test();
    }

    /// lintian `Screen/Python/Egg/Metadata.pm` masks the hint for a file in a
    /// `*.egg-info/` folder that has PKG-INFO beside it. METADATA does not count
    /// for an egg-info folder, and PKG-INFO does not count for a dist-info one.
    #[test]
    fn egg_info_documentation_is_masked_beside_pkg_info() {
        let pass = vec![folder(&[
            ("usr/lib/python3/dist-packages/foo.egg-info/PKG-INFO", "p"),
            ("usr/lib/python3/dist-packages/foo.egg-info/NOTES.txt", "n"),
        ])];
        let fail = vec![
            folder(&[("usr/lib/python3/dist-packages/foo.egg-info/NOTES.txt", "n")]),
            folder(&[
                (
                    "usr/lib/python3/dist-packages/foo-1.0.dist-info/PKG-INFO",
                    "p",
                ),
                (
                    "usr/lib/python3/dist-packages/foo-1.0.dist-info/NOTES.txt",
                    "n",
                ),
            ]),
        ];
        BinaryPackageTester::new(PackageContainsDocumentationOutsideUsrShareDoc, pass, fail).test();
    }

    /// lintian `Check/Documentation.pm` exempts a README whose text contains
    /// "this directory", matched with `m{this directory}xi`. The `x` flag drops
    /// the space from the pattern, so only the run-together text exempts a file.
    #[test]
    fn readme_this_directory_exemption_needs_no_space() {
        let pass = vec![case("usr/share/example/README", "ThisDirectory is empty")];
        let fail = vec![
            case("usr/share/example/README", "This directory holds shims."),
            case("usr/share/example/README.md", "This directory holds shims."),
        ];
        BinaryPackageTester::new(PackageContainsDocumentationOutsideUsrShareDoc, pass, fail).test();
    }
}
