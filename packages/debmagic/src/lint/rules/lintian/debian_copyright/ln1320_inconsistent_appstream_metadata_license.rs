use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::{SourceContext, follow_symlinks};
use crate::lint::debian_copyright::CopyrightParagraph;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;
use crate::lint::xml::XmlDocument;

use super::dep5::license_identifier;
use super::dep5_scan::wildcards;

pub struct InconsistentAppstreamMetadataLicense;

declare_rule! {
    /// An AppStream metadata file names a `metadata_license` that its
    /// `debian/copyright` paragraph does not.
    ///
    /// Each `.xml` file that a `Files` pattern of a copyright paragraph covers
    /// is compared. The paragraph that wins the file (the last pattern to
    /// cover it, as for the wildcard checks) gives the licenses expected. The
    /// file's `/component/metadata_license`, lowercased, must match one of
    /// them, where `mit` is `expat`, `-or-later` is `+`, `-only` is dropped,
    /// and `.0` is ignored. Files that do not parse, have no
    /// `metadata_license`, or leave it empty are skipped, and so are the
    /// `.pc/` files and, in lintian's own package, paths containing `t/`.
    /// Extra is `<file> (<license> != <expected>)` and the Location is the
    /// copyright file, with no line. Symlinks are followed, as lintian does.
    InconsistentAppstreamMetadataLicense,
    code = "LN1320",
    tag = "inconsistent-appstream-metadata-license",
    default_selected = true,
    experimental = false,
}

impl SourceRule for InconsistentAppstreamMetadataLicense {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let copyrights = ctx.debian_copyrights();
        let Some(shipped) = ctx.shipped_files() else {
            return;
        };
        let is_lintian = ctx
            .debian_control()
            .and_then(|control| control.source().get("Source"))
            == Some("lintian");

        // The expected licenses per copyright file, by covered file.
        let mut plans: Vec<(PathBuf, BTreeMap<String, Vec<String>>)> = Vec::new();
        for (path, copyright) in &copyrights {
            let Some(followers) = copyright.paragraphs().and_then(|all| all.get(1..)) else {
                continue;
            };
            let found = wildcards(followers, shipped.file_names());
            let mut expected = BTreeMap::new();
            for (name, wildcard) in &found.wildcard_by_file {
                let sections = found
                    .sections_by_wildcard
                    .get(wildcard)
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                let licenses = sections
                    .iter()
                    .map(|&index| section_license(&followers[index]))
                    .collect();
                expected.insert(name.clone(), licenses);
            }
            plans.push((path.clone(), expected));
        }

        let names: Vec<&String> = plans
            .iter()
            .flat_map(|(_, expected)| expected.keys())
            .filter(|name| name.ends_with(".xml"))
            .filter(|name| !name.starts_with(".pc/"))
            .filter(|name| !(is_lintian && name.contains("t/")))
            .collect();
        let mut licenses: BTreeMap<String, String> = BTreeMap::new();
        if let Some(files) = ctx.files() {
            for name in names {
                let Some(entry) = files.get(name) else {
                    continue;
                };
                let Some(entry) = follow_symlinks(files, entry) else {
                    continue;
                };
                let Some(document) = XmlDocument::read(files, entry) else {
                    continue;
                };
                if document.parse_error().is_some() {
                    continue;
                }
                let Some(license) = document.metadata_license() else {
                    continue;
                };
                let license = license.to_lowercase();
                if !license.is_empty() {
                    licenses.insert(name.clone(), license);
                }
            }
        }

        for (path, expected) in plans {
            for (name, wanted) in &expected {
                let Some(seen) = licenses.get(name) else {
                    continue;
                };
                for want in wanted {
                    if !same_license(seen, want) {
                        ctx.diagnostic(
                            Diagnostic::warn(format!("{name} ({seen} != {want})")).with_location(
                                Location {
                                    path: path.clone(),
                                    line: None,
                                    column: None,
                                },
                            ),
                        );
                    }
                }
            }
        }
    }
}

/// The lowercased short license of a paragraph's `License` field, or empty
/// when it has none.
fn section_license(paragraph: &CopyrightParagraph) -> String {
    if paragraph.declares("License") {
        license_identifier(paragraph.untrimmed_value("License"))
    } else {
        String::new()
    }
}

/// Whether `seen` (the metadata license) and `want` (the copyright license)
/// name the same license, after the normalisation of lintian's `Dep5.pm`.
fn same_license(seen: &str, want: &str) -> bool {
    let seen = normalized(seen);
    let want = normalized(want);
    let (seen_nozero, want_nozero) = (seen.replace(".0", ""), want.replace(".0", ""));
    want == seen || want_nozero == seen || want == seen_nozero || want_nozero == seen_nozero
}

fn normalized(license: &str) -> String {
    let license = if license == "mit" { "expat" } else { license };
    if let Some(head) = license.strip_suffix("-or-later") {
        format!("{head}+")
    } else if let Some(head) = license.strip_suffix("-only") {
        head.to_string()
    } else {
        license.to_string()
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase, TestFile};

    use super::*;

    fn copyright(license: &str) -> TestFile {
        TestFile::new(format!(
            "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n\nFiles: *\nCopyright: 2024 Someone\nLicense: {license}\n"
        ))
    }

    fn metadata(license: &str) -> TestFile {
        TestFile::new(format!(
            "<?xml version=\"1.0\"?>\n<component>\n  <metadata_license>{license}</metadata_license>\n</component>\n"
        ))
    }

    fn tree(license: &str, metadata_license: &str) -> TestCase {
        TestCase::source_tree([
            ("debian/copyright", copyright(license)),
            ("debian/appstream.metadata.xml", metadata(metadata_license)),
        ])
    }

    #[test]
    fn test() {
        let pass = vec![
            TestCase::source_tree(Vec::<(&str, TestFile)>::new()),
            tree("MIT", "MIT"),
            tree("MIT", "expat"),
            tree("GPL-2+", "GPL-2.0+"),
            tree("GPL-2+", "gpl-2-or-later"),
            tree("GPL-2", "gpl-2-only"),
            // Empty or missing metadata_license is skipped.
            tree("MIT", ""),
            TestCase::source_tree([
                ("debian/copyright", copyright("MIT")),
                (
                    "debian/notes.xml",
                    TestFile::new("<component><metadata_license>cc0</metadata_license>"),
                ),
            ]),
        ];
        let fail = vec![
            tree("MIT", "GPL-2+"),
            tree("GPL-2+", "MIT"),
            tree("GPL-2+", "does-not-match-anything"),
        ];
        SourceTreeTester::new(InconsistentAppstreamMetadataLicense, pass, fail).test_and_snapshot();
    }
}
