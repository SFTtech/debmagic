use std::collections::{BTreeMap, HashMap, HashSet};

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::{ShippedFiles, SourceContext};
use crate::lint::debian_copyright::CopyrightParagraph;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct SuperfluousFilePattern;

declare_rule! {
    /// A wildcard in a machine-readable `debian/copyright` matches no shipped file.
    ///
    /// Reads `debian/copyright` and `debian/<installable>.copyright` that are
    /// copyright-format 1.0 (or a post-overhaul draft) and parse. Wildcards are
    /// matched against the shipped files (Source tree: all files; Source
    /// package: Orig files plus Patched `debian/`, or Patched files when Native).
    /// A `Files` wildcard is superfluous when it wins for no file, is not
    /// repeated in the file, and is not redundant with another wildcard of its
    /// paragraph; later paragraphs win. Wildcards with an escape other than
    /// `\*` or `\?` never match. Not evaluated when a `Files` field contains a
    /// comma and no shipped file does. A `Files-Included` wildcard in the
    /// header paragraph is superfluous when it matches nothing; a directory
    /// name there stands for its contents. Extra is the wildcard (prefixed by
    /// `(Files-Included)` for that field); the Location is the copyright file
    /// and the line of the field.
    SuperfluousFilePattern,
    code = "LN0008",
    tag = "superfluous-file-pattern",
    default_selected = true,
    experimental = false,
}

impl SourceRule for SuperfluousFilePattern {
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
            for report in superfluous_patterns(paragraphs, &shipped) {
                ctx.diagnostic(Diagnostic::warn(report.extra).with_location(Location {
                    path: path.clone(),
                    line: report.line,
                    column: None,
                }));
            }
        }
    }
}

struct Report {
    extra: String,
    line: Option<u32>,
}

fn superfluous_patterns(paragraphs: &[CopyrightParagraph], shipped: &ShippedFiles) -> Vec<Report> {
    let Some((header, followers)) = paragraphs.split_first() else {
        return Vec::new();
    };
    let mut reports = unmatched_files_included(header, shipped);
    reports.extend(unmatched_files(followers, shipped));
    reports
}

/// `Files-Included` wildcards of the header that match nothing.
fn unmatched_files_included(header: &CopyrightParagraph, shipped: &ShippedFiles) -> Vec<Report> {
    let mut reports = Vec::new();
    for wildcard in header.trimmed_list("Files-Included") {
        if has_illegal_escape(wildcard) {
            continue;
        }
        let mut wildcard = wildcard.to_string();
        // A bare directory name also covers what is inside it.
        if !wildcard.contains(['*', '?']) {
            let directory = wildcard.trim_end_matches('/');
            if shipped.has_directory(directory) {
                wildcard = format!("{directory}/*");
            }
        }
        if match_glob(&wildcard, shipped.file_names()).is_empty() {
            reports.push(Report {
                extra: format!("(Files-Included) {wildcard}"),
                line: header.position("Files-Included"),
            });
        }
    }
    reports
}

/// `Files` wildcards of the following paragraphs that win for no file.
fn unmatched_files(followers: &[CopyrightParagraph], shipped: &ShippedFiles) -> Vec<Report> {
    let names = shipped.file_names();
    let fields_with_comma = followers
        .iter()
        .any(|paragraph| paragraph.value("Files").contains(','));
    let names_with_comma = names.iter().any(|name| name.contains(','));
    if fields_with_comma && !names_with_comma {
        return Vec::new();
    }

    let mut paragraphs_by_wildcard: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    let mut wildcard_by_file: HashMap<&str, &str> = HashMap::new();
    let mut redundant: HashSet<&str> = HashSet::new();

    for (index, paragraph) in followers.iter().enumerate() {
        // The first paragraph may be the (no longer optional) implied `Files: *`.
        let wildcards = if index == 0
            && paragraph.declares("License")
            && paragraph.declares("Copyright")
            && !paragraph.declares("Files")
        {
            vec!["*"]
        } else {
            paragraph.trimmed_list("Files")
        };
        for wildcard in &wildcards {
            paragraphs_by_wildcard
                .entry(wildcard)
                .or_default()
                .push(index);
        }

        let mut wildcards_by_file: HashMap<&str, Vec<&str>> = HashMap::new();
        for wildcard in &wildcards {
            if has_illegal_escape(wildcard) {
                continue;
            }
            for name in match_glob(wildcard, names) {
                wildcards_by_file.entry(name).or_default().push(wildcard);
            }
        }
        for (name, matching) in wildcards_by_file {
            // Later wildcards and later paragraphs take precedence.
            wildcard_by_file.insert(name, matching[matching.len() - 1]);
            if matching.len() > 1 {
                redundant.extend(matching);
            }
        }
    }

    let used: HashSet<&str> = wildcard_by_file.values().copied().collect();
    let mut reports = Vec::new();
    for (wildcard, indexes) in &paragraphs_by_wildcard {
        if used.contains(wildcard) || indexes.len() > 1 || redundant.contains(wildcard) {
            continue;
        }
        reports.extend(indexes.iter().map(|&index| Report {
            extra: (*wildcard).to_string(),
            line: followers[index].position("Files"),
        }));
    }
    reports
}

/// Lintian's `escape_errors`: any backslash sequence other than `\*` and `\?`.
fn has_illegal_escape(wildcard: &str) -> bool {
    let mut chars = wildcard.chars();
    while let Some(character) = chars.next() {
        if character == '\\' && !matches!(chars.next(), Some('*' | '?')) {
            return true;
        }
    }
    false
}

/// Lintian's `match_glob` (`Regexp::Wildcards` jokers): `*` matches anything,
/// slashes and leading dots included, `?` any one character, `\*` and `\?`
/// are literal. The whole name must match.
fn match_glob<'a>(wildcard: &str, names: &'a [String]) -> Vec<&'a str> {
    let wildcard = wildcard.strip_prefix("./").unwrap_or(wildcard);
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
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const FORMAT: &str =
        "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n";

    fn copyright(paragraphs: &str) -> String {
        format!("{FORMAT}\n{paragraphs}")
    }

    fn section(files: &str) -> String {
        format!("Files: {files}\nCopyright: 2024 Someone\nLicense: MIT\n\n")
    }

    fn tree(copyright: String, others: &[&str]) -> Vec<(String, TestFile)> {
        let mut files = vec![("debian/copyright".to_string(), TestFile::new(copyright))];
        files.extend(
            others
                .iter()
                .map(|path| (path.to_string(), TestFile::new("content"))),
        );
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(copyright(&section("*")), &["a", "b/c"]),
            // `*` crosses slashes and leading dots.
            tree(copyright(&section("*")), &[".hidden", "dir/.hidden"]),
            tree(copyright(&section("a")), &["a"]),
            // `*` also covers `debian/`.
            tree(copyright(&section("deb*")), &["a"]),
            tree(copyright(&section("./a")), &["a"]),
            tree(copyright(&section("a?c")), &["abc"]),
            tree(copyright(&section("a\\*c")), &["a*c"]),
            tree(copyright(&section("a\\?c")), &["a?c"]),
            tree(copyright(&section("a [b] (c)")), &["a", "[b]", "(c)"]),
            // Later paragraphs win, but both win something.
            tree(
                copyright(&format!("{}{}", section("*"), section("debian/*"))),
                &["a"],
            ),
            // Not machine-readable, outdated, or unreadable files are not looked at.
            tree("Copyright: me\nFiles: nothing\n".to_string(), &["a"]),
            tree(
                "Format: http://wiki.debian.org/Proposals/CopyrightFormat\n\nFiles: nothing\n"
                    .to_string(),
                &["a"],
            ),
            tree(copyright("Files: a\nFiles: b\n"), &["a"]),
            // Comma-separated lists are not evaluated unless a file name has a comma.
            tree(copyright(&section("a,b")), &["a", "b"]),
            // The implied `Files: *` of the first paragraph.
            tree(
                copyright("Copyright: 2024 Someone\nLicense: MIT\n\n"),
                &["a"],
            ),
            // A stand-alone license paragraph has no wildcards.
            tree(
                copyright(&format!("{}License: MIT\n text\n", section("*"))),
                &["a"],
            ),
            // `Files-Included`.
            tree(
                format!("{FORMAT}Files-Included: a\n\n{}", section("*")),
                &["a"],
            ),
            // The extra file for an installable is read too.
            vec![
                (
                    "debian/control".to_string(),
                    TestFile::new(
                        "Source: example\n\nPackage: foo\nArchitecture: all\nDescription: foo\n",
                    ),
                ),
                (
                    "debian/foo.copyright".to_string(),
                    TestFile::new(copyright(&section("a"))),
                ),
                ("a".to_string(), TestFile::new("x")),
            ],
        ];
        let fail = vec![
            tree(copyright(&section("missing")), &["a"]),
            tree(copyright(&section("xeb*")), &["a"]),
            tree(copyright(&section("a?c")), &["abbc"]),
            // The whole name must match.
            tree(copyright(&section("a")), &["ab", "ba", "dir/a"]),
            // Escapes other than `\*` and `\?` never match.
            tree(copyright(&section("a\\n")), &["a\\n", "an"]),
            tree(copyright(&section("a\\\\b")), &["a\\b"]),
            tree(copyright(&section("a\\")), &["a"]),
            tree(copyright(&section("a\\*")), &["abc"]),
            // A later paragraph takes everything the earlier one matched.
            tree(
                copyright(&format!("{}{}", section("deb*"), section("debian/*"))),
                &["debian/x"],
            ),
            tree(
                copyright(&format!("{}{}", section("a"), section("*"))),
                &["a"],
            ),
            // Several wildcards in one paragraph: only the unused one is reported.
            tree(copyright(&section("a b")), &["a"]),
            tree(copyright(&section("a\n b")), &["a"]),
            // Commas are evaluated when a file name has a comma.
            tree(copyright(&section("a,b")), &["a,c"]),
            tree(
                copyright(&format!("{}{}", section("a,b"), section("nothing"))),
                &["a,b"],
            ),
            // The only paragraph after the header is a follower.
            tree(copyright(&section("nothing")), &[]),
            tree(
                format!("{FORMAT}Files-Included: nothing\n\n{}", section("*")),
                &["a"],
            ),
            tree(
                format!("{FORMAT}Files-Included: dir\n\n{}", section("*")),
                &["a"],
            ),
            vec![
                (
                    "debian/control".to_string(),
                    TestFile::new(
                        "Source: example\n\nPackage: foo\nArchitecture: all\nDescription: foo\n",
                    ),
                ),
                (
                    "debian/foo.copyright".to_string(),
                    TestFile::new(copyright(&section("missing"))),
                ),
            ],
        ];
        SourceTreeTester::new(SuperfluousFilePattern, pass, fail).test_and_snapshot();
    }

    #[test]
    fn duplicates_and_redundant_wildcards_are_not_reported() {
        let pass = vec![
            // The same wildcard in two paragraphs.
            tree(
                copyright(&format!("{}{}", section("missing"), section("missing"))),
                &["a"],
            ),
            // The same wildcard twice in one paragraph.
            tree(copyright(&section("missing missing")), &["a"]),
            // Two wildcards of one paragraph matching the same file.
            tree(copyright(&section("a a*")), &["abc", "a"]),
        ];
        SourceTreeTester::new(
            SuperfluousFilePattern,
            pass,
            Vec::<Vec<(String, TestFile)>>::new(),
        )
        .test();
    }

    #[test]
    fn files_included_directory_stands_for_its_contents() {
        let with_directory = |files: &str, directory: &str| {
            let mut tree = tree(
                format!("{FORMAT}Files-Included: {files}\n\n{}", section("*")),
                &[],
            );
            tree.push((directory.to_string(), TestFile::directory()));
            tree.push((format!("{directory}/x"), TestFile::new("x")));
            tree
        };
        let pass = vec![
            with_directory("dir", "dir"),
            with_directory("dir/", "dir"),
            with_directory("dir/*", "dir"),
        ];
        let fail = vec![
            // Not a directory: the pattern stays as written.
            with_directory("other", "dir"),
        ];
        SourceTreeTester::new(SuperfluousFilePattern, pass, fail).test();
    }

    #[test]
    fn source_package_ships_orig_and_patched_debian() {
        let dsc = |format: &str| {
            Dsc::parse(&format!(
                "Format: {format}\nSource: example\nVersion: 1.0-1\n"
            ))
            .expect("Dsc")
        };
        let patched = |copyright_files: &str| {
            vec![
                (
                    "debian/copyright".to_string(),
                    TestFile::new(copyright(&section(copyright_files))),
                ),
                ("upstream-patched".to_string(), TestFile::new("p")),
            ]
        };
        let pass = vec![
            // Native: the Patched files are shipped.
            SourcePackageCase::from(patched("upstream-patched")),
            // Non-native: Orig files.
            SourcePackageCase::from(patched("upstream"))
                .orig(vec![("upstream", "o")])
                .dsc(dsc("3.0 (quilt)")),
            // Non-native: the Patched `debian/` is added.
            SourcePackageCase::from(patched("debian/*"))
                .orig(vec![("upstream", "o")])
                .dsc(dsc("3.0 (quilt)")),
            // Non-native 1.0 keeps `debian/` from the Orig files.
            SourcePackageCase::from(patched("debian/orig-only"))
                .orig(vec![("debian/orig-only", "o")])
                .dsc(dsc("1.0")),
        ];
        let fail = vec![
            // Files a patch adds are not shipped in the Orig files.
            SourcePackageCase::from(patched("upstream-patched"))
                .orig(vec![("upstream", "o")])
                .dsc(dsc("3.0 (quilt)")),
            // `3.0 (quilt)` drops `debian/` from the Orig files.
            SourcePackageCase::from(patched("debian/orig-only"))
                .orig(vec![("debian/orig-only", "o")])
                .dsc(dsc("3.0 (quilt)")),
            SourcePackageCase::from(patched("missing")),
        ];
        SourcePackageTester::new(SuperfluousFilePattern, pass, fail).test_and_snapshot();
    }
}
