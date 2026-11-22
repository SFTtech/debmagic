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

const ORIGIN_TEMPLATES: [&str; 2] = [
    "(upstream|backport|vendor|other), (<patch-url>|commit:<commit-id>)",
    "<vendor|upstream|other>, <url of original patch>",
];

const BUG_TEMPLATE: &str = "<url in upstream bugtracker>";

pub(super) struct Dep3Header {
    origin: String,
    bug: String,
    bug_debian: String,
    last_update: String,
    forwarded: String,
    applied_upstream: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum BugProblem {
    DefaultValue,
    DebianNumber,
    NotUrl,
}

impl Dep3Header {
    /// Origin still holds one of the template values.
    pub(super) fn origin_is_template(&self) -> bool {
        ORIGIN_TEMPLATES.contains(&self.origin.as_str())
    }

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

    /// Origin is not `upstream` or `backport`, and Forwarded is `no`, or none of
    /// Applied-Upstream, Bug and Forwarded is set.
    pub(super) fn not_forwarded_upstream(&self) -> bool {
        let category = self.origin.split(',').next().unwrap_or_default().trim();
        if matches!(category, "upstream" | "backport") {
            return false;
        }
        self.forwarded == "no"
            || [&self.applied_upstream, &self.bug, &self.forwarded]
                .iter()
                .all(|field| field.is_empty())
    }

    /// Set but not a `YYYY-MM-DD` date that exists and is not before 1900.
    pub(super) fn last_update_is_wrong(&self) -> bool {
        !self.last_update.is_empty() && !is_rfc3339_date(&self.last_update)
    }

    /// Set but not a Debian BTS or salsa.debian.org http or https URL.
    pub(super) fn bug_debian_is_wrong(&self) -> bool {
        !self.bug_debian.is_empty() && !is_debian_url(&self.bug_debian)
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
        origin: value("origin").to_string(),
        bug: value("bug").to_string(),
        bug_debian: value("bug-debian").to_string(),
        last_update: value("last-update").to_string(),
        forwarded: value("forwarded").to_string(),
        applied_upstream: value("applied-upstream").to_string(),
    })
}

/// Lintian accepts any URI whose scheme is http or https.
fn is_http_url(value: &str) -> bool {
    value.split_once(':').is_some_and(|(scheme, _)| {
        scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
    })
}

/// Lintian's `not_debian_url` negated: an http or https URL whose host is
/// `bugs.debian.org` or `salsa.debian.org`, compared case-insensitively. The
/// authority is cut by hand (userinfo and port dropped), not by a URI parser.
fn is_debian_url(value: &str) -> bool {
    let Some((scheme, rest)) = value.split_once(':') else {
        return false;
    };
    if !(scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")) {
        return false;
    }
    let Some(after_slashes) = rest.strip_prefix("//") else {
        return false;
    };
    let authority = after_slashes
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    let host_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host_port)| host_port);
    let host = host_port
        .rsplit_once(':')
        .map_or(host_port, |(host, _)| host);
    host.eq_ignore_ascii_case("bugs.debian.org") || host.eq_ignore_ascii_case("salsa.debian.org")
}

/// Lintian's `check_wrong_rfc3339_date` negated. Lintian converts the date with
/// `Time::Local` and compares the round trip in the local zone, so a midnight
/// that does not exist there (a DST gap) would fail in lintian and passes here.
fn is_rfc3339_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let digits = |range: std::ops::Range<usize>| -> Option<u32> {
        let part = &bytes[range];
        if !part.iter().all(u8::is_ascii_digit) {
            return None;
        }
        std::str::from_utf8(part).ok()?.parse().ok()
    };
    let (Some(year), Some(month), Some(day)) = (digits(0..4), digits(5..7), digits(8..10)) else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => return false,
    };
    year >= 1900 && (1..=days_in_month).contains(&day)
}

fn is_debian_number(value: &str) -> bool {
    value
        .strip_prefix('#')
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}
