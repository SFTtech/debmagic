//! Body of `debian/watch`, matching lintian 2.139.0 `Check::Debian::Watch`.
//!
//! The standard line is the one `Watch.pm` reads (first `version` / `Version`
//! line), which is not the `debian/watch/standard` classification. Info and
//! pedantic tags from that check are not Rules. A standard below 2 stops the
//! walk after any invalid version line.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::Severity;

const SIGNING_KEYS: &[&str] = &[
    "debian/upstream-signing-key.pgp",
    "debian/upstream/signing-key.pgp",
    "debian/upstream/signing-key.asc",
];

static VERSION_ANY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*version\s*[:=]\s*([0-9]+)\s*$").expect("valid regex"));
static VERSION_1_TO_4: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*version\s*=\s*([1-4])\s*$").expect("valid regex"));
static VERSION_5_PLUS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*version\s?[:=]\s*([5-9]|[1-9][0-9]+)$").expect("valid regex")
});
static OPTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"opt(?:ion)?s=(?:"((?:[^"]|\\")+)"|(\S+))\s+"#).expect("valid regex")
});
static REPACK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"dfsg|debian|ds|repack").expect("valid regex"));
static EPOCH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9]+:").expect("valid regex"));
static PAREN_COMPONENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/([^/]*\([^/]*\)[^/]*)$").expect("valid regex"));
static SOURCEFORGE_HOST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?s)(?:https?|ftp)://(?:(?:.+\.)?dl|(?:pr)?downloads?|ftp[0-9]?|upload)\.(?:sourceforge|sf)\.net",
    )
    .expect("valid regex")
});
static SOURCEFORGE_SHOWFILES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"https?://(?:www\.)?(?:sourceforge|sf)\.net/project/showfiles\.php")
        .expect("valid regex")
});
static SOURCEFORGE_FILES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"https?://(?:www\.)?(?:sourceforge|sf)\.net/projects/.+/files")
        .expect("valid regex")
});
static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\w").expect("valid regex"));
static COMMA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*,\s*").expect("valid regex"));
static V5_LINES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*\r?\n\s*").expect("valid regex"));
static TEMPLATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)CRAN|Bioconductor").expect("valid regex"));

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum WatchFileHit {
    LineInvalid(String),
    LacksSourceforgeRedirector(String),
    DeprecatedGithubredir(String),
    DeprecatedSfRedirector(String),
    NotManglingVersion(String),
    WrongUpstreamVersion(String),
    OldUpstreamVersion(String),
    PubkeyFileIsMissing,
    CouldVerifyDownload(String),
}

impl WatchFileHit {
    fn extra(&self) -> String {
        match self {
            Self::LineInvalid(extra)
            | Self::LacksSourceforgeRedirector(extra)
            | Self::DeprecatedGithubredir(extra)
            | Self::DeprecatedSfRedirector(extra)
            | Self::NotManglingVersion(extra)
            | Self::WrongUpstreamVersion(extra)
            | Self::OldUpstreamVersion(extra)
            | Self::CouldVerifyDownload(extra) => extra.clone(),
            Self::PubkeyFileIsMissing => String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WatchFileFinding {
    pub line: Option<u32>,
    pub hit: WatchFileHit,
}

pub(super) struct WatchInput {
    pub text: String,
    pub native: bool,
    /// Upstream version used for the repack marker. Empty for a native package.
    pub upstream: String,
    /// Changelog versions, newest first. `None` when there is no changelog.
    pub versions: Option<Vec<String>>,
    pub keyfile: Option<String>,
}

pub(super) fn flag_watch_file(
    ctx: &mut SourceContext<'_>,
    severity: Severity,
    accept: impl Fn(&WatchFileHit) -> bool,
) {
    let Some((path, findings)) = watch_file_from_context(ctx) else {
        return;
    };
    for finding in findings {
        if !accept(&finding.hit) {
            continue;
        }
        let diagnostic = match severity {
            Severity::Error => Diagnostic::error(finding.hit.extra()),
            _ => Diagnostic::warn(finding.hit.extra()),
        };
        ctx.diagnostic(diagnostic.with_location(Location {
            path: path.clone(),
            line: finding.line,
            column: None,
        }));
    }
}

fn watch_file_from_context(
    ctx: &mut SourceContext<'_>,
) -> Option<(std::path::PathBuf, Vec<WatchFileFinding>)> {
    let (path, text, format, keyfile) = {
        let files = ctx.files()?;
        let entry = files.get("debian/watch")?;
        if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
            return None;
        }
        let path = entry.path().to_path_buf();
        let text = String::from_utf8_lossy(&files.read(entry).ok()?).into_owned();
        let format = read_text(files, "debian/source/format");
        let keyfile = SIGNING_KEYS.iter().find_map(|name| {
            let entry = files.get(*name)?;
            matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
                .then(|| entry.path().to_string_lossy().into_owned())
        });
        (path, text, format, keyfile)
    };
    let versions = ctx.debian_changelog().map(|changelog| {
        changelog
            .entries()
            .iter()
            .map(|entry| entry.version().to_string())
            .collect::<Vec<_>>()
    });
    let format = format.or_else(|| {
        ctx.dsc()
            .and_then(|dsc| dsc.fields().get("Format"))
            .map(str::to_string)
    });
    let native = is_native(
        format.as_deref(),
        versions
            .as_ref()
            .and_then(|v| v.first().map(String::as_str)),
    );
    let upstream = if native {
        String::new()
    } else {
        versions
            .as_ref()
            .and_then(|versions| versions.first())
            .map(|version| mangle_version(version))
            .unwrap_or_default()
    };
    let findings = watch_file_findings(&WatchInput {
        text,
        native,
        upstream,
        versions,
        keyfile,
    });
    Some((path, findings))
}

fn read_text(files: &FileTree, path: &str) -> Option<String> {
    let entry = files.get(path)?;
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return None;
    }
    let bytes = files.read(entry).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

pub(super) fn watch_file_findings(input: &WatchInput) -> Vec<WatchFileFinding> {
    let mut findings = Vec::new();
    let contents = strip_final_backslash(&input.text);
    let lines = perl_lines(&contents);
    let mut standard = None;
    for (index, line) in lines.iter().enumerate() {
        let Some(caps) = VERSION_ANY.captures(line) else {
            continue;
        };
        let mut version = caps[1].to_string();
        if VERSION_1_TO_4.is_match(line) || VERSION_5_PLUS.is_match(line) {
            if let Some(caps) = VERSION_1_TO_4
                .captures(line)
                .or_else(|| VERSION_5_PLUS.captures(line))
            {
                version = caps[1].to_string();
            }
        } else {
            findings.push(WatchFileFinding {
                line: Some((index + 1) as u32),
                hit: WatchFileHit::LineInvalid(line.trim_end().to_string()),
            });
        }
        standard = version.parse::<u32>().ok();
        break;
    }
    let Some(standard) = standard else {
        return findings;
    };
    if standard < 2 {
        return findings;
    }

    let repack = REPACK.find(&input.upstream).map(|m| m.as_str().to_string());
    let mut with_pgp = false;
    let mut dversions = BTreeSet::new();
    let mut continued = String::new();
    let mut persisted_lastversion: Option<String> = None;

    for (index, raw) in lines.iter().enumerate() {
        let position = (index + 1) as u32;
        let mut line = raw.trim_start().to_string();
        if line.starts_with('#') {
            line.clear();
        }
        if standard < 5 && line.is_empty() {
            continued.clear();
            continue;
        }
        if standard < 5 {
            if let Some(stripped) = line.strip_suffix('\\') {
                continued.push_str(stripped);
                continue;
            }
        } else if !line.is_empty() {
            continued.push_str(&line);
            continued.push('\n');
            if index + 1 < lines.len() {
                continue;
            }
        }
        if !continued.is_empty() {
            if standard >= 5 {
                line = continued.clone();
            } else {
                line = format!("{continued}{line}");
            }
        }
        let version_line = VERSION_ANY.is_match(&line);
        if standard < 5 {
            continued.clear();
            if version_line {
                continue;
            }
        } else if version_line {
            continued.clear();
            continue;
        } else if position < lines.len() as u32 {
            continue;
        }

        let mut remainder = line.clone();
        let mut options: Vec<String> = Vec::new();
        if standard < 5 {
            if let Some(caps) = OPTS.captures(&remainder) {
                let body = caps
                    .get(1)
                    .or_else(|| caps.get(2))
                    .map(|m| m.as_str())
                    .unwrap_or_default();
                options = COMMA
                    .split(body)
                    .filter(|part| !part.is_empty())
                    .map(str::to_string)
                    .collect();
                let matched = caps.get(0).expect("whole match");
                let mut rest = String::new();
                rest.push_str(&remainder[..matched.start()]);
                rest.push_str(&remainder[matched.end()..]);
                remainder = rest;
            }
        } else {
            options = V5_LINES
                .split(&line)
                .filter(|part| WORD.is_match(part))
                .map(str::to_string)
                .collect();
        }

        if standard < 5 && remainder.is_empty() {
            findings.push(WatchFileFinding {
                line: Some(position),
                hit: WatchFileHit::LineInvalid(line.trim_end().to_string()),
            });
            continue;
        }

        let mut repack_mangle = false;
        let mut repack_dmangle_auto = false;
        for option in &options {
            let mut option = option.clone();
            let mut template = String::new();
            if standard >= 5 {
                let (normalized, template_value) = normalize_v5_option(&option);
                option = normalized;
                template = template_value;
            }
            if let Some(word) = &repack {
                let cran = TEMPLATE.is_match(&template);
                if let Some(value) = mangle_value(&option, false)
                    && (value.starts_with("auto") || value.contains(word) || cran)
                {
                    repack_mangle = true;
                }
            }
            if standard >= 4 && dversion_contains_auto(&option) {
                repack_dmangle_auto = true;
            }
            if sets_pgp(&option) {
                with_pgp = true;
            }
        }

        if remainder.contains("qa.debian.org/watch/sf.php?") {
            findings.push(WatchFileFinding {
                line: Some(position),
                hit: WatchFileHit::DeprecatedSfRedirector(remainder.clone()),
            });
        }
        if remainder.contains("githubredir.debian.net") {
            findings.push(WatchFileFinding {
                line: Some(position),
                hit: WatchFileHit::DeprecatedGithubredir(remainder.clone()),
            });
        }
        if SOURCEFORGE_HOST.is_match(&remainder)
            || SOURCEFORGE_SHOWFILES.is_match(&remainder)
            || SOURCEFORGE_FILES.is_match(&remainder)
        {
            findings.push(WatchFileFinding {
                line: Some(position),
                hit: WatchFileHit::LacksSourceforgeRedirector(remainder.clone()),
            });
        }

        if !remainder.is_empty() {
            let fields = split_space(&remainder, 4);
            if let Some(base) = fields.first() {
                let explicit = if PAREN_COMPONENT.is_match(base) {
                    split_space(&remainder, 3)
                        .get(1)
                        .map(|part| (*part).to_string())
                } else {
                    fields.get(2).map(|part| (*part).to_string())
                };
                if let Some(version) = &explicit {
                    dversions.insert(version.clone());
                }
                persisted_lastversion = Some(explicit.unwrap_or_else(|| "debian".to_string()));
            }
        }
        let lastversion = persisted_lastversion.as_deref();
        let needs_repack = repack.is_some() && (standard >= 5 || lastversion == Some("debian"));
        if needs_repack && !repack_mangle && !repack_dmangle_auto {
            findings.push(WatchFileFinding {
                line: Some(position),
                hit: WatchFileHit::NotManglingVersion(line),
            });
        }
    }

    if let Some(keyfile) = &input.keyfile {
        if !with_pgp {
            findings.push(WatchFileFinding {
                line: None,
                hit: WatchFileHit::CouldVerifyDownload(keyfile.clone()),
            });
        }
    } else if with_pgp {
        findings.push(WatchFileFinding {
            line: None,
            hit: WatchFileHit::PubkeyFileIsMissing,
        });
    }

    if let Some(versions) = &input.versions
        && !dversions.is_empty()
    {
        let mut orig = BTreeSet::new();
        let mut mangled: BTreeMap<String, u32> = BTreeMap::new();
        for (index, version) in versions.iter().enumerate() {
            let count = (index + 1) as u32;
            orig.insert(version.clone());
            mangled.entry(mangle_version(version)).or_insert(count);
        }
        for dversion in &dversions {
            if dversion == "debian" {
                continue;
            }
            if !input.native && orig.contains(dversion) {
                findings.push(WatchFileFinding {
                    line: None,
                    hit: WatchFileHit::WrongUpstreamVersion(dversion.clone()),
                });
                continue;
            }
            if mangled.get(dversion).is_some_and(|count| *count != 1) {
                findings.push(WatchFileFinding {
                    line: None,
                    hit: WatchFileHit::OldUpstreamVersion(dversion.clone()),
                });
            }
        }
    }
    findings
}

fn strip_final_backslash(contents: &str) -> String {
    let (body, newline) = contents
        .strip_suffix('\n')
        .map(|body| (body, true))
        .unwrap_or((contents, false));
    if body.ends_with('\\') && !body.ends_with("\\\\") {
        let mut stripped = body[..body.len() - 1].to_string();
        if newline {
            stripped.push('\n');
        }
        stripped
    } else {
        contents.to_string()
    }
}

fn perl_lines(contents: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = contents.split('\n').collect();
    while lines.last() == Some(&"") {
        lines.pop();
    }
    lines
}

fn split_space(text: &str, limit: usize) -> Vec<&str> {
    text.splitn(limit, ' ').collect()
}

fn mangle_version(version: &str) -> String {
    let without_revision = match version.rfind('-') {
        Some(index) => &version[..index],
        None => version,
    };
    EPOCH.replace(without_revision, "").into_owned()
}

fn is_native(format: Option<&str>, version: Option<&str>) -> bool {
    match format.map(str::trim) {
        Some("3.0 (native)") => true,
        Some("3.0 (quilt)") => false,
        Some(other) if other.starts_with("1.0") => {
            version.is_some_and(|version| !version.contains('-'))
        }
        None => version.is_some_and(|version| !version.contains('-')),
        Some(_) => false,
    }
}

/// `dversion_only` selects `dversionmangle`. Otherwise `[ud]?versionmangle`.
fn mangle_value(option: &str, dversion_only: bool) -> Option<&str> {
    let rest = if dversion_only {
        option.strip_prefix("dversionmangle")?
    } else {
        option
            .strip_prefix("uversionmangle")
            .or_else(|| option.strip_prefix("dversionmangle"))
            .or_else(|| option.strip_prefix("versionmangle"))?
    };
    Some(rest.trim_start().strip_prefix('=')?.trim_start())
}

fn dversion_contains_auto(option: &str) -> bool {
    mangle_value(option, true)
        .is_some_and(|value| value.contains("s/@DEB_EXT@/") || value.contains("auto"))
}

fn sets_pgp(option: &str) -> bool {
    if let Some(rest) = option.strip_prefix("pgpsigurlmangle") {
        return rest.trim_start().starts_with('=');
    }
    if let Some(rest) = option.strip_prefix("pgpmode") {
        let Some(rest) = rest.trim_start().strip_prefix('=') else {
            return false;
        };
        let value = rest.trim();
        return !value.is_empty() && value != "none";
    }
    false
}

fn normalize_v5_option(option: &str) -> (String, String) {
    let option = option.trim_end_matches(['\r', '\n']);
    let Some((key, value)) = option.split_once(':') else {
        return (option.to_string(), String::new());
    };
    let value = value.trim_start();
    if key.is_empty() || value.is_empty() {
        return (option.to_string(), String::new());
    }
    let key: String = key.chars().filter(|c| *c != '-').collect();
    let template = if key.eq_ignore_ascii_case("template") {
        value.to_string()
    } else {
        String::new()
    };
    (format!("{}={value}", key.to_lowercase()), template)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use crate::lint::debian_changelog::DebianChangelog;

    use super::*;

    const TAGS: &[&str] = &[
        "debian-watch-line-invalid",
        "debian-watch-lacks-sourceforge-redirector",
        "debian-watch-file-uses-deprecated-githubredir",
        "debian-watch-file-uses-deprecated-sf-redirector-method",
        "debian-watch-not-mangling-version",
        "debian-watch-file-specifies-wrong-upstream-version",
        "debian-watch-file-specifies-old-upstream-version",
        "debian-watch-file-pubkey-file-is-missing",
        "debian-watch-could-verify-download",
    ];

    #[test]
    fn vendored_watch_recipes_match_hint_extras() {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/lintian-parity/checks/debian/watch");
        let mut checked = 0;
        for entry in fs::read_dir(&root).expect("watch recipes") {
            let dir = entry.expect("recipe dir").path();
            let watch = dir.join("debian/watch");
            if !watch.is_file() {
                continue;
            }
            let hints = fs::read_to_string(dir.join("eval/hints")).unwrap_or_default();
            let expected = hint_extras(&hints);
            if expected.is_empty() && !TAGS.iter().any(|tag| hints.contains(tag)) {
                continue;
            }
            let text = fs::read_to_string(&watch).expect("watch");
            let format = fs::read_to_string(dir.join("debian/source/format")).ok();
            let changelog = DebianChangelog::load(&dir.join("debian/changelog"));
            let versions = changelog.map(|changelog| {
                changelog
                    .entries()
                    .iter()
                    .map(|entry| entry.version().to_string())
                    .collect::<Vec<_>>()
            });
            let native = is_native(
                format.as_deref(),
                versions
                    .as_ref()
                    .and_then(|versions| versions.first().map(String::as_str)),
            );
            let upstream = if native {
                String::new()
            } else {
                versions
                    .as_ref()
                    .and_then(|versions| versions.first())
                    .map(|version| mangle_version(version))
                    .unwrap_or_default()
            };
            let keyfile = SIGNING_KEYS
                .iter()
                .find_map(|name| dir.join(name).is_file().then(|| (*name).to_string()));
            let got = finding_extras(&watch_file_findings(&WatchInput {
                text,
                native,
                upstream,
                versions,
                keyfile,
            }));
            assert_eq!(got, expected, "recipe {}", dir.display());
            checked += 1;
        }
        assert!(checked > 0, "no watch recipes exercised these tags");
    }

    fn finding_extras(findings: &[WatchFileFinding]) -> Vec<(String, String)> {
        let mut extras = Vec::new();
        for finding in findings {
            let tag = match &finding.hit {
                WatchFileHit::LineInvalid(_) => "debian-watch-line-invalid",
                WatchFileHit::LacksSourceforgeRedirector(_) => {
                    "debian-watch-lacks-sourceforge-redirector"
                }
                WatchFileHit::DeprecatedGithubredir(_) => {
                    "debian-watch-file-uses-deprecated-githubredir"
                }
                WatchFileHit::DeprecatedSfRedirector(_) => {
                    "debian-watch-file-uses-deprecated-sf-redirector-method"
                }
                WatchFileHit::NotManglingVersion(_) => "debian-watch-not-mangling-version",
                WatchFileHit::WrongUpstreamVersion(_) => {
                    "debian-watch-file-specifies-wrong-upstream-version"
                }
                WatchFileHit::OldUpstreamVersion(_) => {
                    "debian-watch-file-specifies-old-upstream-version"
                }
                WatchFileHit::PubkeyFileIsMissing => "debian-watch-file-pubkey-file-is-missing",
                WatchFileHit::CouldVerifyDownload(_) => "debian-watch-could-verify-download",
            };
            extras.push((tag.to_string(), finding.hit.extra().trim().to_string()));
        }
        extras.sort();
        extras
    }

    fn hint_extras(hints: &str) -> Vec<(String, String)> {
        let mut extras = Vec::new();
        for line in hints.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((_, rest)) = line.split_once(": ") else {
                continue;
            };
            let mut fields = rest.splitn(2, char::is_whitespace);
            let Some(tag) = fields.next() else {
                continue;
            };
            if !TAGS.contains(&tag) {
                continue;
            }
            let extra = strip_hint_pointer(fields.next().unwrap_or(""));
            extras.push((tag.to_string(), extra));
        }
        extras.sort();
        extras
    }

    fn strip_hint_pointer(extra: &str) -> String {
        let extra = extra.trim();
        if let Some((before, pointer)) = extra.rsplit_once(" [")
            && pointer.ends_with(']')
        {
            return before.trim().to_string();
        }
        if extra.starts_with('[') && extra.ends_with(']') {
            return String::new();
        }
        extra.to_string()
    }
}
