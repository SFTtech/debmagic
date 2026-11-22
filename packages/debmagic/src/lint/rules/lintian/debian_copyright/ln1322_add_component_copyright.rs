use std::collections::BTreeSet;
use std::path::PathBuf;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_copyright::{CopyrightParagraph, parse_paragraphs};
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::dep5_scan::copyright_texts;

pub struct AddComponentCopyright;

declare_rule! {
    /// The source ships an extra orig component that no copyright paragraph
    /// names.
    ///
    /// The components come from the `.orig-<component>.tar` names in the
    /// `.dsc` `Files` field of a Source package; a Source tree has no `.dsc`
    /// and reports nothing. A component is missing when no `Files` pattern in
    /// a paragraph after the header starts with its directory (the part before
    /// the first `/`, with no wildcard). Each copyright file (`debian/copyright`
    /// and `debian/<installable>.copyright`, not symlinks, UTF-8 only) gets one
    /// report per missing component, with the component as extra. Only a file
    /// that parses as Deb822 paragraphs is read. Info, not selected by default.
    AddComponentCopyright,
    code = "LN1322",
    tag = "add-component-copyright",
    default_selected = false,
    experimental = false,
}

impl SourceRule for AddComponentCopyright {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let components = match ctx.dsc() {
            Some(dsc) => extra_components(dsc.fields()),
            None => return,
        };
        if components.is_empty() {
            return;
        }
        for (path, text) in copyright_texts(ctx) {
            let Ok(paragraphs) = parse_paragraphs(&text) else {
                continue;
            };
            let Some((_header, followers)) = paragraphs.split_first() else {
                continue;
            };
            let covered = initial_components(followers);
            for component in components.iter().filter(|name| !covered.contains(*name)) {
                ctx.diagnostic(
                    Diagnostic::warn(component.clone())
                        .with_severity(Severity::Info)
                        .with_location(Location {
                            path: PathBuf::from(&path),
                            line: None,
                            column: None,
                        }),
                );
            }
        }
    }
}

/// Non-empty component names of the orig tarballs listed in the `.dsc`,
/// sorted and without repeats. Names follow lintian's
/// `$pkg_$version.orig(-$comp)?.tar.$ext` pattern, with the epoch dropped and
/// the Debian revision stripped for `orig`; the native `$pkg_$version.tar.$ext`
/// has no component.
fn extra_components(fields: &crate::lint::debian_control::ControlParagraph) -> BTreeSet<String> {
    let source = fields.get("Source").unwrap_or_default().trim();
    let version = fields.get("Version").unwrap_or_default().trim();
    let noepoch = match version.split_once(':') {
        Some((_, rest)) if !rest.is_empty() => rest,
        _ => version,
    };
    let baserev = format!("{source}_{noepoch}");
    let upstream = match noepoch.rfind('-') {
        Some(index) if index > 0 => &noepoch[..index],
        _ => noepoch,
    };
    let base = format!("{source}_{upstream}");
    let pattern = format!(
        r"^(?:{}\.orig(?:-(.*))?|{})\.tar\.(?:gz|bz2|lzma|xz)$",
        regex::escape(&base),
        regex::escape(&baserev),
    );
    let Ok(tarball) = Regex::new(&pattern) else {
        return BTreeSet::new();
    };
    let Some(files) = fields.get("Files") else {
        return BTreeSet::new();
    };
    files
        .lines()
        .filter_map(|line| line.split_whitespace().last())
        .filter_map(|name| tarball.captures(name))
        .filter_map(|captures| {
            captures
                .get(1)
                .map(|component| component.as_str().to_string())
        })
        .filter(|component| !component.is_empty())
        .collect()
}

/// Directories that the `Files` patterns of the paragraphs after the header
/// name outright: the part before the first `/`, when it has no wildcard.
fn initial_components(followers: &[CopyrightParagraph]) -> BTreeSet<String> {
    followers
        .iter()
        .flat_map(|paragraph| paragraph.trimmed_list("Files"))
        .map(|pattern| pattern.split('/').next().unwrap_or_default())
        .filter(|directory| !directory.is_empty() && !directory.contains(['*', '?']))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester};

    use super::*;

    const COPYRIGHT: &str = "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n\n\
        Files: *\nCopyright: 2024 Someone\nLicense: MIT\n";

    fn dsc(files: &str) -> Dsc {
        Dsc::parse(&format!(
            "Format: 3.0 (quilt)\nSource: example\nVersion: 1.0-1\n\
             Maintainer: Example <ex@example.com>\nFiles:\n{files}"
        ))
        .expect("Dsc fixture")
    }

    fn package(copyright: &str, files: &str) -> SourcePackageCase {
        SourcePackageCase::from(vec![
            ("debian/copyright", copyright.to_string()),
            (
                "debian/control",
                "Source: example\nMaintainer: Example <ex@example.com>\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n".to_string(),
            ),
        ])
        .dsc(dsc(files))
    }

    #[test]
    fn source_tree() {
        let pass = vec![vec![("debian/copyright", COPYRIGHT)]];
        SourceTreeTester::new(AddComponentCopyright, pass, Vec::new()).test();
    }

    #[test]
    fn source_package() {
        let orig = " d41d8cd98f00b204e9800998ecf8427e 0 example_1.0.orig.tar.gz\n";
        let signed = " d41d8cd98f00b204e9800998ecf8427e 0 example_1.0.orig-signed.tar.xz\n";
        let noext = " d41d8cd98f00b204e9800998ecf8427e 0 example_1.0.orig-noext.tar.bz2\n";
        let pass = vec![
            package(COPYRIGHT, orig),
            package(
                "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n\n\
                 Files: *\nCopyright: 2024 Someone\nLicense: MIT\n\n\
                 Files: signed/*\nCopyright: 2024 Someone\nLicense: MIT\n",
                &format!("{orig}{signed}"),
            ),
        ];
        let fail = vec![
            package(COPYRIGHT, &format!("{orig}{signed}")),
            package(COPYRIGHT, &format!("{orig}{signed}{noext}")),
        ];
        SourcePackageTester::new(AddComponentCopyright, pass, fail).test_and_snapshot();
    }
}
