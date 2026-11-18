//! DEP-3 header fields of the patches under `debian/patches/`, by line scan.
//!
//! Mirrors the gates of lintian's `debian/patches/dep3` Check: the header is
//! the text before the first line starting with `---`, a patch with neither
//! Description nor Subject, or with none of Origin, Author and From, yields
//! nothing. Field names are case-insensitive and the last mention wins.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::lint::context::SourceContext;
use crate::lint::file_tree::EntryKind;

const BUG_TEMPLATE: &str = "<url in upstream bugtracker>";

pub(super) struct Dep3Header {
    bug: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum BugProblem {
    DefaultValue,
    DebianNumber,
    NotUrl,
}

impl Dep3Header {
    /// Set but not an http or https URL.
    pub(super) fn bug_problem(&self) -> Option<BugProblem> {
        if self.bug.is_empty() || is_http_url(&self.bug) {
            return None;
        }
        Some(if self.bug == BUG_TEMPLATE {
            BugProblem::DefaultValue
        } else if is_debian_number(&self.bug) {
            BugProblem::DebianNumber
        } else {
            BugProblem::NotUrl
        })
    }
}

/// Parsed headers of the patches that lintian's Check reads, with their paths.
pub(super) fn dep3_headers(ctx: &mut SourceContext<'_>) -> Vec<(PathBuf, Dep3Header)> {
    let Some(files) = ctx.files() else {
        return Vec::new();
    };
    let patches: Vec<(PathBuf, Vec<u8>)> = files
        .entries()
        .iter()
        .filter(|entry| entry.kind() == EntryKind::RegularFile)
        .filter(|entry| entry.path().starts_with("debian/patches"))
        .filter(|entry| {
            !matches!(
                entry.path().to_str(),
                Some("debian/patches/series" | "debian/patches/README")
            )
        })
        .filter_map(|entry| {
            let bytes = files.read(entry).ok()?;
            Some((entry.path().to_path_buf(), bytes))
        })
        .collect();
    patches
        .into_iter()
        .filter_map(|(path, bytes)| Some((path, parse_header(&bytes)?)))
        .collect()
}

fn parse_header(bytes: &[u8]) -> Option<Dep3Header> {
    let mut end = 0;
    for line in bytes.split_inclusive(|&byte| byte == b'\n') {
        if line.starts_with(b"---") {
            break;
        }
        end += line.len();
    }
    let header = std::str::from_utf8(&bytes[..end]).ok()?;

    let mut fields: HashMap<String, String> = HashMap::new();
    let mut current: Option<String> = None;
    for line in header.lines() {
        if line.trim().is_empty() {
            current = None;
        } else if line.starts_with([' ', '\t']) {
            if let Some(key) = &current {
                let value = fields.entry(key.clone()).or_default();
                value.push('\n');
                value.push_str(line.trim());
            }
        } else {
            match line.split_once(':') {
                Some((key, value)) if !key.is_empty() && !key.contains(char::is_whitespace) => {
                    let key = key.to_ascii_lowercase();
                    fields.insert(key.clone(), value.trim().to_string());
                    current = Some(key);
                }
                _ => current = None,
            }
        }
    }

    let value = |name: &str| fields.get(name).map(|v| v.trim()).unwrap_or_default();
    if value("description").is_empty() && value("subject").is_empty() {
        return None;
    }
    if value("origin").is_empty() && value("author").is_empty() && value("from").is_empty() {
        return None;
    }
    Some(Dep3Header {
        bug: value("bug").to_string(),
    })
}

/// Lintian accepts any URI whose scheme is http or https.
fn is_http_url(value: &str) -> bool {
    value.split_once(':').is_some_and(|(scheme, _)| {
        scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
    })
}

fn is_debian_number(value: &str) -> bool {
    value
        .strip_prefix('#')
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}
