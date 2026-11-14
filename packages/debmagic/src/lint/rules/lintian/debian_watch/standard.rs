//! `debian/watch` version line, matching lintian's `debian/watch/standard`.
//!
//! Classification `debian-watch-file-standard` and info
//! `older-debian-watch-file-standard` are not Rules. Known standards are 2, 3,
//! 4, and 5. A standard below 4 is obsolete. The `version=` form wins when any
//! line uses it; `Version:` is read only when `version=` is absent.

use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;

const KNOWN: &[&str] = &["2", "3", "4", "5"];

static VERSION_EQ: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^version\s*=\s*(\d+)\s*$").expect("valid regex"));
static VERSION_COLON: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^Version\s*:\s*(\d+)\s*$").expect("valid regex"));
static HAS_CONTENTS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*[^#]").expect("valid regex"));

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum WatchHit {
    Missing,
    Multiple(String),
    Unknown(String),
    Obsolete(String),
}

pub(super) fn watch_hits(text: &str) -> Vec<WatchHit> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut mentioned: Vec<String> = VERSION_EQ
        .captures_iter(text)
        .map(|caps| caps[1].to_string())
        .collect();
    if mentioned.is_empty() {
        mentioned = VERSION_COLON
            .captures_iter(text)
            .map(|caps| caps[1].to_string())
            .collect();
    }
    let has_contents = HAS_CONTENTS.is_match(text);
    if has_contents && mentioned.is_empty() {
        return vec![WatchHit::Missing];
    }
    let mut hits = Vec::new();
    if mentioned.len() > 1 {
        hits.push(WatchHit::Multiple(mentioned.join(" ")));
    }
    let mut seen = Vec::new();
    for version in &mentioned {
        if !KNOWN.contains(&version.as_str()) && !seen.contains(version) {
            seen.push(version.clone());
            hits.push(WatchHit::Unknown(version.clone()));
        }
    }
    let known: Vec<&str> = mentioned
        .iter()
        .filter(|version| KNOWN.contains(&version.as_str()))
        .map(String::as_str)
        .collect();
    if let Some(highest) = known
        .iter()
        .copied()
        .max_by_key(|version| version.parse::<u32>().unwrap_or(0))
        && highest.parse::<u32>().unwrap_or(0) < 4
    {
        hits.push(WatchHit::Obsolete(highest.to_string()));
    }
    hits
}

pub(super) fn flag_watch(
    ctx: &mut SourceContext<'_>,
    accept: impl Fn(&WatchHit) -> Option<String>,
) {
    let prepared = {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(entry) = files.get("debian/watch") else {
            return;
        };
        if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
            return;
        }
        let Ok(bytes) = files.read(entry) else {
            return;
        };
        let path = entry.path().to_path_buf();
        let text = String::from_utf8_lossy(&bytes).into_owned();
        (path, watch_hits(&text))
    };
    for message in prepared.1.iter().filter_map(accept) {
        ctx.diagnostic(Diagnostic::warn(message).with_location(Location {
            path: prepared.0.clone(),
            line: None,
            column: None,
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_watch_file() {
        let text = "\
version=2
http://example.com

version=3
http://example.com

version=42
";
        assert_eq!(
            watch_hits(text),
            vec![
                WatchHit::Multiple("2 3 42".to_string()),
                WatchHit::Unknown("42".to_string()),
                WatchHit::Obsolete("3".to_string()),
            ]
        );
    }

    #[test]
    fn missing_when_there_is_no_version_line() {
        assert_eq!(
            watch_hits("http://example.com/dist file.tar.gz\n"),
            vec![WatchHit::Missing]
        );
        assert_eq!(watch_hits("\n"), vec![WatchHit::Missing]);
        assert!(watch_hits("").is_empty());
        assert!(watch_hits("# only a comment\n").is_empty());
    }

    #[test]
    fn version_colon_is_ignored_when_version_eq_is_present() {
        assert_eq!(
            watch_hits("version=5\nVersion: 1\n"),
            Vec::<WatchHit>::new()
        );
    }

    #[test]
    fn version_four_is_not_obsolete() {
        assert!(watch_hits("version=4\nhttp://example.com\n").is_empty());
        assert_eq!(
            watch_hits("version=2\n"),
            vec![WatchHit::Obsolete("2".to_string())]
        );
    }
}
