use std::path::Path;
use std::sync::LazyLock;

use chrono::DateTime;
use regex::Regex;

/// One parser error from `Lintian::Changelog`, as `[line, message]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangelogParseError {
    line: u32,
    message: String,
}

impl ChangelogParseError {
    pub fn line(&self) -> u32 {
        self.line
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Parsed `debian/changelog` with lintian's grammar errors.
///
/// Missing files are unavailable. A file that is not UTF-8 is treated as
/// unavailable (lintian skips parse). Syntax errors stay on the object so
/// `syntax-error-in-debian-changelog` can emit them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DebianChangelog {
    errors: Vec<ChangelogParseError>,
}

impl DebianChangelog {
    pub fn load(path: &Path) -> Option<Self> {
        if !path.is_file() {
            return None;
        }
        let bytes = std::fs::read(path).ok()?;
        let text = std::str::from_utf8(&bytes).ok()?;
        Some(Self::parse(text))
    }

    pub fn parse(contents: &str) -> Self {
        Self {
            errors: parse_errors(contents),
        }
    }

    pub fn errors(&self) -> &[ChangelogParseError] {
        &self.errors
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Expect {
    FirstHeading,
    NextHeadingOrEof,
    StartOfChangeData,
    MoreChangeDataOrTrailer,
}

impl Expect {
    fn as_lintian_str(self) -> &'static str {
        match self {
            Self::FirstHeading => "first heading",
            Self::NextHeadingOrEof => "next heading or eof",
            Self::StartOfChangeData => "start of change data",
            Self::MoreChangeDataOrTrailer => "more change data or trailer",
        }
    }
}

#[derive(Default)]
struct EntryScratch {
    source: bool,
    version: bool,
    changes: bool,
    maintainer: bool,
    date: bool,
    timestamp: bool,
}

impl EntryScratch {
    fn is_empty(&self) -> bool {
        !(self.changes || self.source || self.version || self.maintainer || self.date)
    }
}

fn parse_errors(contents: &str) -> Vec<ChangelogParseError> {
    static LOOKS_LIKE_CHANGELOG: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?mx)^ \S+ \s* [(] [^\)]+ [)] \s* (?:[^ \t;]+ \s*)+ ; ").expect("regex")
    });
    static HEADING: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(?P<source>\w[-+0-9a-z.]*) \((?P<version>[^() \t]+)\)(?P<distribution>(?:\s+[-+0-9a-z.]+)+);\s*(?P<kvpairs>.*)$")
            .expect("regex")
    });
    static KV: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^([-0-9a-z]+)=\s*(.*\S)$").expect("regex"));
    static URGENCY: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^([-0-9a-z]+)((\s+.*)?)$").expect("regex"));
    static XBCS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^X[BCS]+-").expect("regex"));
    static TRAILER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^ -- (?P<name>.*) <(?P<email>.*)>(?P<sep>  ?)(?P<date>(?:\w+,\s*)?\d{1,2}\s+\w+\s+\d{4}\s+\d{1,2}:\d{2}:\d{2}\s+[-+]\d{4}(?:\s+\([^\\()]\))?)$")
            .expect("regex")
    });
    static LOCAL_VARIABLES: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^(?:;;\s*)?Local variables:").expect("regex"));
    static OLD_GNU: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?:\w+\s+\w+\s+\d{1,2} \d{1,2}:\d{1,2}:\d{1,2}\s+[\w\s]*\d{4})\s+(?:.*)\s+[<(](?:.*)[)>]")
            .expect("regex")
    });
    static OLD_SHORT_DATE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?:\w+\s+\w+\s+\d{1,2},?\s*\d{4})\s+(?:.*)\s+[<(](?:.*)[)>]").expect("regex")
    });
    static OLD_HEADING: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^(?:\w[-+0-9a-z.]*) \((?:[^() \t]+)\)\;?").expect("regex")
    });
    static OLD_DEBIAN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^(?:[\w.+-]+)[- ]\S+ Debian \S+").expect("regex"));
    static OLD_CHANGES_FROM: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)^Changes from version (?:.*) to (?:.*):").expect("regex")
    });
    static OLD_CHANGES_FOR: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)^Changes for [\w.+-]+-[\w.+-]+:?$").expect("regex"));
    static OLD_VERSION_ONLY: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(?:\d+:)?\w[\w.+~-]*:?$").expect("regex"));
    static CHANGE_DATA: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\s{2,}(\S)").expect("regex"));
    static MALFORMED_TRAILER: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^ --").expect("regex"));
    static CVS_KEYWORD: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\$\w+:.*\$").expect("regex"));
    static BLOCK_COMMENT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^/\*.*\*/").expect("regex"));

    if !LOOKS_LIKE_CHANGELOG.is_match(contents) {
        return vec![ChangelogParseError {
            line: 1,
            message: "not a Debian changelog".to_string(),
        }];
    }

    let mut errors = Vec::new();
    let mut expect = Expect::FirstHeading;
    let mut entry = EntryScratch::default();

    let mut lines: Vec<&str> = contents.split('\n').collect();
    while lines.last() == Some(&"") {
        lines.pop();
    }
    for (index, raw_line) in lines.iter().enumerate() {
        let position = (index + 1) as u32;
        let line = raw_line.trim_end();

        if let Some(heading) = HEADING.captures(line) {
            if expect != Expect::FirstHeading && expect != Expect::NextHeadingOrEof {
                errors.push(ChangelogParseError {
                    line: position,
                    message: format!(
                        "found start of entry where expected {}",
                        expect.as_lintian_str()
                    ),
                });
            }
            entry = EntryScratch::default();
            entry.source = true;
            entry.version = heading.name("version").is_some();
            parse_kvpairs(
                heading.name("kvpairs").map(|m| m.as_str()).unwrap_or(""),
                position,
                &mut errors,
                &KV,
                &URGENCY,
                &XBCS,
            );
            expect = Expect::StartOfChangeData;
        } else if LOCAL_VARIABLES.is_match(line) || line.to_ascii_lowercase().starts_with("vim:") {
            break;
        } else if CVS_KEYWORD.is_match(line)
            || line.starts_with("# ")
            || BLOCK_COMMENT.is_match(line)
        {
            continue;
        } else if OLD_GNU.is_match(line)
            || OLD_SHORT_DATE.is_match(line)
            || OLD_HEADING.is_match(line)
            || OLD_DEBIAN.is_match(line)
            || OLD_CHANGES_FROM.is_match(line)
            || OLD_CHANGES_FOR.is_match(line)
            || line.eq_ignore_ascii_case("Old Changelog:")
            || OLD_VERSION_ONLY.is_match(line)
        {
            break;
        } else if line.starts_with(|ch: char| !ch.is_whitespace()) {
            errors.push(ChangelogParseError {
                line: position,
                message: "badly formatted heading line".to_string(),
            });
        } else if let Some(trailer) = TRAILER.captures(line) {
            if expect != Expect::MoreChangeDataOrTrailer {
                errors.push(ChangelogParseError {
                    line: position,
                    message: format!("found trailer where expected {}", expect.as_lintian_str()),
                });
            }
            if trailer.name("sep").map(|m| m.as_str()) != Some("  ") {
                errors.push(ChangelogParseError {
                    line: position,
                    message: "badly formatted trailer line".to_string(),
                });
            }
            entry.maintainer = true;
            if !(entry.date && entry.timestamp) {
                let date = trailer.name("date").map(|m| m.as_str()).unwrap_or("");
                entry.date = true;
                if parse_changelog_timestamp(date) {
                    entry.timestamp = true;
                } else {
                    errors.push(ChangelogParseError {
                        line: position,
                        message: format!("could not parse date {date}"),
                    });
                }
            }
            expect = Expect::NextHeadingOrEof;
        } else if MALFORMED_TRAILER.is_match(line) {
            errors.push(ChangelogParseError {
                line: position,
                message: "badly formatted trailer line".to_string(),
            });
        } else if CHANGE_DATA.is_match(line) {
            if expect != Expect::StartOfChangeData && expect != Expect::MoreChangeDataOrTrailer {
                errors.push(ChangelogParseError {
                    line: position,
                    message: format!(
                        "found change data where expected {}",
                        expect.as_lintian_str()
                    ),
                });
                if expect == Expect::NextHeadingOrEof && !entry.is_empty() {
                    entry = EntryScratch {
                        source: true,
                        version: true,
                        ..EntryScratch::default()
                    };
                }
            }
            entry.changes = true;
            expect = Expect::MoreChangeDataOrTrailer;
        } else if !line.contains(|ch: char| !ch.is_whitespace()) {
            if expect == Expect::StartOfChangeData || expect == Expect::NextHeadingOrEof {
                continue;
            }
            if expect != Expect::MoreChangeDataOrTrailer {
                errors.push(ChangelogParseError {
                    line: position,
                    message: format!(
                        "found blank line where expected {}",
                        expect.as_lintian_str()
                    ),
                });
            }
        } else {
            errors.push(ChangelogParseError {
                line: position,
                message: "unrecognised line".to_string(),
            });
            if expect == Expect::StartOfChangeData || expect == Expect::MoreChangeDataOrTrailer {
                entry.changes = true;
                expect = Expect::MoreChangeDataOrTrailer;
            }
        }
    }

    if expect != Expect::NextHeadingOrEof {
        let position = (lines.len() + 1) as u32;
        errors.push(ChangelogParseError {
            line: position,
            message: format!("found eof where expected {}", expect.as_lintian_str()),
        });
    }

    errors
}

fn parse_kvpairs(
    kvpairs: &str,
    position: u32,
    errors: &mut Vec<ChangelogParseError>,
    kv: &Regex,
    urgency: &Regex,
    xbcs: &Regex,
) {
    let mut seen = Vec::new();
    for raw in kvpairs.split(',') {
        let pair = raw.trim();
        if pair.is_empty() {
            continue;
        }
        let Some(captures) = kv.captures(pair) else {
            errors.push(ChangelogParseError {
                line: position,
                message: format!("bad key-value after ';': '{pair}'"),
            });
            continue;
        };
        let key_raw = captures.get(1).map(|m| m.as_str()).unwrap_or("");
        let mut key_chars = key_raw.chars();
        let key = match key_chars.next() {
            Some(first) => {
                let mut s = first.to_uppercase().collect::<String>();
                s.push_str(key_chars.as_str());
                s
            }
            None => String::new(),
        };
        let value = captures.get(2).map(|m| m.as_str()).unwrap_or("");
        if seen.iter().any(|existing| existing == &key) {
            errors.push(ChangelogParseError {
                line: position,
                message: format!("repeated key-value {key}"),
            });
        } else {
            seen.push(key.clone());
        }
        if key == "Urgency" {
            if !urgency.is_match(value) {
                errors.push(ChangelogParseError {
                    line: position,
                    message: format!("badly formatted urgency value {value}"),
                });
            }
        } else if xbcs.is_match(&key) {
        } else if key != "Binary-only" {
            errors.push(ChangelogParseError {
                line: position,
                message: format!("unknown key-value key {key} - copying to XS-{key}"),
            });
        }
    }
}

fn parse_changelog_timestamp(date: &str) -> bool {
    let date = date
        .rsplit_once(" (")
        .and_then(|(before, rest)| rest.ends_with(')').then_some(before))
        .unwrap_or(date);
    const FORMATS: &[&str] = &[
        "%a, %d %b %Y %H:%M:%S %z",
        "%a, %e %b %Y %H:%M:%S %z",
        "%d %b %Y %H:%M:%S %z",
        "%e %b %Y %H:%M:%S %z",
        "%a, %-d %b %Y %H:%M:%S %z",
        "%-d %b %Y %H:%M:%S %z",
    ];
    FORMATS
        .iter()
        .any(|format| DateTime::parse_from_str(date, format).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn messages(contents: &str) -> Vec<(u32, String)> {
        DebianChangelog::parse(contents)
            .errors()
            .iter()
            .map(|error| (error.line(), error.message().to_string()))
            .collect()
    }

    #[test]
    fn empty_is_not_a_debian_changelog() {
        assert_eq!(
            messages(""),
            vec![(1, "not a Debian changelog".to_string())]
        );
    }

    #[test]
    fn upstream_changelog_is_not_debian() {
        assert_eq!(
            messages(
                "2011-09-10  Niels Thykier  <niels@thykier.net>\n\n\t* This is a non-Debian ChangeLog.\n"
            ),
            vec![(1, "not a Debian changelog".to_string())]
        );
    }

    #[test]
    fn valid_changelog_has_no_errors() {
        assert!(messages(
            "example (1.0) unstable; urgency=low\n\n  * test\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 -0800\n",
        )
        .is_empty());
    }

    #[test]
    fn empty_entry_matches_lintian() {
        assert_eq!(
            messages(
                "example (1.0) unstable; urgency=low\n\n  * .\n  *\n\n -- A Uthor <a@example.com> Tue, 30 Dec 2008 17:34:02 -0800\n\nexample (0.1) unstable; urgency=low\n\n -- A Uthor <a@example.com>  Fri, 06 Feb 2009 22:22:37 -0800\n",
            ),
            vec![
                (6, "badly formatted trailer line".to_string()),
                (
                    10,
                    "found trailer where expected start of change data".to_string()
                ),
            ]
        );
    }

    #[test]
    fn empty_version_is_badly_formatted_heading() {
        assert_eq!(
            messages(
                "example (1.0) unstable; urgency=low\n\n  * Lintian Test Suite.\n  * Test: changelog-file-syntax\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 -0800\n\nexample (0.9) unstable; urgency=low\n\n  * Lintian Test Suite.\n  * Test: changelog-file-syntax\n\n -- A Uthor <a@example.com>  Sat, 09 Apr 2016 10:56:49 +0000\n\nexample () unstable; urgency=low\n\n  * Lintian Test Suite.\n  * Test: changelog-file-syntax\n\n  * Suppress \"should close ITP bug\" messages.  (Closes: #123456)\n\n -- A Uthor <a@example.com>  Sat, 02 Apr 2016 10:56:49 +0000\n",
            ),
            vec![
                (15, "badly formatted heading line".to_string()),
                (
                    17,
                    "found change data where expected next heading or eof".to_string()
                ),
            ]
        );
    }

    #[test]
    fn unparseable_date_matches_lintian() {
        assert_eq!(
            messages(
                "example (1.0) unstable; urgency=low\n\n  * Lintian Test Suite.\n  * Test: changelog-file-strange-date\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 -0800\n\nexample (1.0) unstable; urgency=low\n\n  * The date will fail with some dpkg version see #794674\n\n -- A Uthor <a@example.com>  The, 15 Apr 2004 23:33:51 +0200\n",
            ),
            vec![(
                12,
                "could not parse date The, 15 Apr 2004 23:33:51 +0200".to_string()
            )]
        );
    }

    #[test]
    fn old_changelog_marker_is_not_an_error() {
        assert!(messages(
            "example (1.0) unstable; urgency=low\n\n  * test\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 -0800\n\nOld Changelog:\n\nexample (0.1) whatever\n",
        )
        .is_empty());
    }

    #[test]
    fn missing_trailer_is_eof_error() {
        assert_eq!(
            messages("example (1.0) unstable; urgency=low\n\n  * test\n"),
            vec![(
                4,
                "found eof where expected more change data or trailer".to_string()
            )]
        );
    }

    #[test]
    fn missing_file_is_unavailable() {
        let path =
            std::env::temp_dir().join(format!("debmagic-no-changelog-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        assert!(DebianChangelog::load(&path).is_none());
    }
}
