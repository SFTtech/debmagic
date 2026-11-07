use std::collections::HashSet;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::{ShippedFiles, SourceContext};
use crate::lint::debian_copyright::CopyrightParagraph;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct SourceShipsExcludedFile;

declare_rule! {
    /// A file matched by `Files-Excluded` is still in the source.
    ///
    /// Machine-readable `debian/copyright` (and `debian/<installable>.copyright`).
    /// A pattern without `*` or `?` that names a shipped directory also covers
    /// its contents. `Files-Included` wins those names back. Paths under
    /// `debian/` or `.pc/` are not reported. A wildcard with an escape other
    /// than `\*` or `\?` is skipped. Extra is the shipped path. The Location
    /// is the `Files-Excluded` line.
    SourceShipsExcludedFile,
    code = "LN0378",
    tag = "source-ships-excluded-file",
    default_selected = true,
    experimental = false,
}

impl SourceRule for SourceShipsExcludedFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let copyrights = ctx.debian_copyrights();
        if copyrights.is_empty() {
            return;
        }
        let Some(shipped) = ctx.shipped_files() else {
            return;
        };
        for (path, copyright) in copyrights {
            let Some(paragraphs) = copyright.paragraphs() else {
                continue;
            };
            let Some(header) = paragraphs.first() else {
                continue;
            };
            let line = header.position("Files-Excluded");
            for name in unwanted(header, &shipped) {
                ctx.diagnostic(Diagnostic::error(name).with_location(Location {
                    path: path.clone(),
                    line,
                    column: None,
                }));
            }
        }
    }
}

fn unwanted(header: &CopyrightParagraph, shipped: &ShippedFiles) -> Vec<String> {
    let excluded = matched(header, "Files-Excluded", shipped);
    let included_names = matched(header, "Files-Included", shipped);
    let included: HashSet<&str> = included_names.iter().map(String::as_str).collect();
    let mut seen = HashSet::new();
    let mut names = Vec::new();
    for name in excluded {
        if name.starts_with("debian/") || name.starts_with(".pc/") {
            continue;
        }
        if included.contains(name.as_str()) {
            continue;
        }
        if seen.insert(name.clone()) {
            names.push(name);
        }
    }
    names
}

fn matched(header: &CopyrightParagraph, field: &str, shipped: &ShippedFiles) -> Vec<String> {
    let mut names = Vec::new();
    for wildcard in header.trimmed_list(field) {
        if has_illegal_escape(wildcard) {
            continue;
        }
        let mut pattern = wildcard.to_string();
        if !pattern.contains(['*', '?']) {
            let mut candidate = pattern.clone();
            if !candidate.ends_with('/') {
                candidate.push('/');
            }
            let directory = candidate.trim_end_matches('/');
            if shipped.has_directory(directory) {
                pattern = format!("{candidate}*");
            }
        }
        names.extend(
            match_glob(&pattern, shipped.file_names())
                .into_iter()
                .map(str::to_string),
        );
    }
    names
}

fn has_illegal_escape(wildcard: &str) -> bool {
    let mut chars = wildcard.chars();
    while let Some(character) = chars.next() {
        if character == '\\' && !matches!(chars.next(), Some('*' | '?')) {
            return true;
        }
    }
    false
}

fn match_glob<'a>(wildcard: &str, names: &'a [String]) -> Vec<&'a str> {
    let wildcard = wildcard.strip_prefix("./").unwrap_or(wildcard);
    if wildcard == "*" {
        return names.iter().map(String::as_str).collect();
    }
    let mut pattern = String::from("^");
    let mut chars = wildcard.chars();
    while let Some(character) = chars.next() {
        match character {
            '*' => pattern.push_str(".*"),
            '?' => pattern.push('.'),
            '\\' => match chars.next() {
                Some(escaped) => pattern.push_str(&regex::escape(&escaped.to_string())),
                None => pattern.push_str(r"\\"),
            },
            other => pattern.push_str(&regex::escape(&other.to_string())),
        }
    }
    pattern.push_str(r"\z");
    let Ok(regex) = Regex::new(&pattern) else {
        return Vec::new();
    };
    names
        .iter()
        .map(String::as_str)
        .filter(|name| regex.is_match(name))
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const FORMAT: &str =
        "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n";
    const CONTROL: &str = "Source: example\nMaintainer: Example <ex@example.com>\nStandards-Version: 4.7.2\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n";

    fn copyright(excluded: &str) -> String {
        format!("{FORMAT}Files-Excluded: {excluded}\n\nFiles: *\nCopyright: X\nLicense: MIT\n")
    }

    fn tree(excluded: &str, paths: &[(&str, TestFile)]) -> Vec<(String, TestFile)> {
        let mut files = vec![
            ("debian/control".to_string(), TestFile::from(CONTROL)),
            (
                "debian/copyright".to_string(),
                TestFile::new(copyright(excluded)),
            ),
        ];
        files.extend(
            paths
                .iter()
                .map(|(path, file)| (path.to_string(), file.clone())),
        );
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            tree("missing", &[("kept", TestFile::new("x"))]),
            tree("debian/*", &[("debian/rules", TestFile::new("x"))]),
            tree(
                "gone\nFiles-Included: gone",
                &[("gone", TestFile::new("x"))],
            ),
            vec![(
                "debian/copyright".to_string(),
                TestFile::new("not machine readable\n"),
            )],
        ];
        let mut directory = tree(
            "exclude-this\n exclude-dir\n exclude-dir-2/*\n .*",
            &[
                ("exclude-this", TestFile::new("x")),
                ("exclude-dir/filename", TestFile::new("x")),
                ("exclude-dir-2/filename", TestFile::new("x")),
                (".matches", TestFile::new("x")),
                ("kept", TestFile::new("x")),
            ],
        );
        directory.push(("exclude-dir".to_string(), TestFile::directory()));
        directory.push(("exclude-dir-2".to_string(), TestFile::directory()));
        let fail = vec![directory];
        SourceTreeTester::new(SourceShipsExcludedFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            "missing",
            &[("kept", TestFile::new("x"))],
        ))];
        let fail = vec![SourcePackageCase::from(tree(
            "some-file",
            &[("some-file", TestFile::new("x"))],
        ))];
        SourcePackageTester::new(SourceShipsExcludedFile, pass, fail).test_and_snapshot();
    }
}
