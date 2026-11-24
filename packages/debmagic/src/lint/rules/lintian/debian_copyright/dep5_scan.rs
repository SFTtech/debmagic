//! Reading of the copyright files that the `debian/copyright/dep5` Check reads,
//! and the matching of their patterns. Shared by the Rules whose Tags need the
//! text or the wildcard matching, mirroring `Dep5.pm`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::{ShippedFiles, SourceContext};
use crate::lint::debian_copyright::{
    CopyrightParagraph, DebianCopyright, Dep5Version, find_dep5_version, format_uri,
};

use super::dep5::follower_wildcards;

/// A Format URI hint of `check_dep5_copyright`, for a machine-readable file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FormatUriHint {
    /// `unknown-copyright-format-uri`: no `Format` field in the first paragraph
    /// (`None`), or a URI that `find_dep5_version` does not recognise.
    Unknown(Option<String>),
    /// A `0~wiki` URI.
    Wiki(String),
    /// A `0+svn` URI without a revision.
    Unversioned(String),
    /// A `0+svn~N` URI with N before the last significant change.
    OutOfDate(String),
}

static BOILERPLATE_URI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:rev=REVISION|VERSIONED_FORMAT_URL)\b").expect("valid regex")
});

/// The Format URI hint of a copyright file, as `check_dep5_copyright` reports
/// it. `None` for a file that is not machine-readable, for a boilerplate URI
/// (its own Tag), and for a URI that needs no hint. The insecure `http://`
/// hint is left to its own Rule.
pub(super) fn format_uri_hint(text: &str) -> Option<FormatUriHint> {
    if DebianCopyright::parse(text) == DebianCopyright::NotMachineReadable {
        return None;
    }
    let Some(uri) = format_uri(text) else {
        return Some(FormatUriHint::Unknown(None));
    };
    if BOILERPLATE_URI.is_match(&uri) {
        return None;
    }
    match find_dep5_version(&uri) {
        None => Some(FormatUriHint::Unknown(Some(uri))),
        Some(Dep5Version::Wiki) => Some(FormatUriHint::Wiki(uri)),
        Some(Dep5Version::Svn(None)) => Some(FormatUriHint::Unversioned(uri)),
        Some(version) if version.is_before_significant_change() => {
            Some(FormatUriHint::OutOfDate(uri))
        }
        Some(_) => None,
    }
}

/// lintian's `processable->repacked`: the upstream part of the changelog
/// version (epoch and Debian revision removed) matches `$PKGREPACK_REGEX`.
pub(super) fn repacked(ctx: &SourceContext<'_>) -> bool {
    let Some(version) = ctx
        .debian_changelog()
        .and_then(|changelog| changelog.entries().first())
        .map(|entry| entry.version().to_string())
    else {
        return false;
    };
    let without_epoch = version.split_once(':').map_or(&*version, |(_, rest)| rest);
    let upstream = without_epoch
        .rsplit_once('-')
        .map_or(without_epoch, |(upstream, _)| upstream);
    ["dfsg", "debian", "ds", "repack"]
        .iter()
        .any(|marker| upstream.contains(marker))
}

/// lintian's `processable->native`, shared with the source format Rules.
pub(super) fn native(ctx: &mut SourceContext<'_>) -> bool {
    super::super::fields_version::is_native(ctx)
}

/// `debian/copyright` and `debian/<installable>.copyright` with their text.
/// Symlinks, non-files and files that are not UTF-8 are left out, as lintian
/// does (`is_symlink`, `is_valid_utf8`). Candidates match `debian_copyrights`.
pub(super) fn copyright_texts(ctx: &mut SourceContext<'_>) -> Vec<(PathBuf, String)> {
    let mut candidates = vec![PathBuf::from("debian/copyright")];
    if let Some(control) = ctx.debian_control() {
        candidates.extend(
            control.installables().iter().map(|installable| {
                PathBuf::from(format!("debian/{}.copyright", installable.name()))
            }),
        );
    }
    let Some(files) = ctx.files() else {
        return Vec::new();
    };
    candidates
        .into_iter()
        .filter_map(|path| {
            let bytes = files.read_file(&path)?;
            let text = std::str::from_utf8(&bytes).ok()?.to_string();
            Some((path, text))
        })
        .collect()
}

/// Illegal escapes of a pattern, in order (`escape_errors`): a backslash with
/// the character after it, unless that character is `*` or `?`.
pub(super) fn escape_errors(wildcard: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut chars = wildcard.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            continue;
        }
        match chars.next() {
            Some('*' | '?') => {}
            Some(escaped) => found.push(format!("\\{escaped}")),
            None => found.push(String::from("\\")),
        }
    }
    found
}

/// Names matched by a pattern (`match_glob`), in the order of `names`.
pub(super) fn match_glob<'a>(wildcard: &str, names: &'a [String]) -> Vec<&'a str> {
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

/// Names of a `Files-Excluded` or `Files-Included` field, as `Dep5.pm` reads
/// them: a pattern with an illegal escape is skipped, and a plain directory
/// name also covers its contents. Repeats are kept.
pub(super) fn matched(
    header: &CopyrightParagraph,
    field: &str,
    shipped: &ShippedFiles,
) -> Vec<String> {
    let mut names = Vec::new();
    for wildcard in header.trimmed_list(field) {
        if !escape_errors(wildcard).is_empty() {
            continue;
        }
        let mut pattern = wildcard.to_string();
        if !pattern.contains(['*', '?']) {
            let candidate = if pattern.ends_with('/') {
                pattern.clone()
            } else {
                format!("{pattern}/")
            };
            if shipped.has_directory(candidate.trim_end_matches('/')) {
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

/// Lintian's `$check_wildcards`: the glob checks run unless a `Files` field
/// has a comma and no shipped file name does.
pub(super) fn check_wildcards(followers: &[CopyrightParagraph], shipped: &[String]) -> bool {
    let commas = followers
        .iter()
        .any(|paragraph| paragraph.value("Files").contains(','));
    !commas || shipped.iter().any(|name| name.contains(','))
}

/// What the paragraph loop of `Dep5.pm` derives from the `Files` patterns.
#[derive(Default)]
pub(super) struct Wildcards {
    /// Paragraph index (into the followers) of each pattern, in order.
    pub sections_by_wildcard: BTreeMap<String, Vec<usize>>,
    /// Last pattern to cover each shipped file. Filled only when the wildcard
    /// checks run.
    pub wildcard_by_file: BTreeMap<String, String>,
    /// Illegal escapes per paragraph index. Only when the wildcard checks run.
    pub escapes: Vec<(usize, String)>,
    /// (paragraph index, loosing pattern, winning pattern, file) for every
    /// file that an earlier pattern covers deeper than the winning pattern.
    pub out_of_order: Vec<(usize, String, String, String)>,
    /// (paragraph index, file, sorted patterns) for every file that two or more
    /// patterns of the same paragraph cover, repeats included.
    pub redundant: Vec<(usize, String, Vec<String>)>,
}

/// Runs the paragraph loop of `Dep5.pm` over the followers of the header.
pub(super) fn wildcards(followers: &[CopyrightParagraph], shipped: &[String]) -> Wildcards {
    let gated = check_wildcards(followers, shipped);
    let mut found = Wildcards::default();
    for (index, paragraph) in followers.iter().enumerate() {
        let patterns = follower_wildcards(paragraph, index);
        for pattern in &patterns {
            found
                .sections_by_wildcard
                .entry(pattern.clone())
                .or_default()
                .push(index);
        }
        if !gated {
            continue;
        }
        let mut covered: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for pattern in &patterns {
            let errors = escape_errors(pattern);
            found
                .escapes
                .extend(errors.iter().map(|error| (index, error.clone())));
            if !errors.is_empty() {
                continue;
            }
            for name in match_glob(pattern, shipped) {
                covered
                    .entry(name.to_string())
                    .or_default()
                    .push(pattern.clone());
            }
        }
        for (name, same) in &covered {
            let (Some(loosing), Some(winning)) = (found.wildcard_by_file.get(name), same.last())
            else {
                continue;
            };
            if depth(loosing) > depth(winning) {
                found
                    .out_of_order
                    .push((index, loosing.clone(), winning.clone(), name.clone()));
            }
        }
        for (name, same) in &covered {
            if same.len() > 1 {
                let mut patterns = same.clone();
                patterns.sort();
                found.redundant.push((index, name.clone(), patterns));
            }
        }
        for (name, same) in covered {
            if let Some(winning) = same.last() {
                found.wildcard_by_file.insert(name, winning.clone());
            }
        }
    }
    found
}

fn depth(pattern: &str) -> usize {
    pattern.matches('/').count()
}
