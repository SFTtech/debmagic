//! NMU / QA facts from the newest changelog trailer and `debian/control`.

use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::SourceContext;

static DOMAIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<([^>@]+@)([A-Za-z0-9_.-]+)>").expect("valid regex"));
static REVISION_NMU: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"-[^.-]+(\.[^.-]+)?(\.[^.-]+)?$").expect("valid regex"));
static PLUS_NMU: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\+nmu\d+$").expect("valid regex"));
static PLUS_BIN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\+b\d+$").expect("valid regex"));
static BACKPORT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"~bpo(\d+)\+(\d+)$").expect("valid regex"));
static STABLE_UPDATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[~|+]deb(\d+)u(\d+)$").expect("valid regex"));
static UPLOADER_SPLIT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r">\s*,\s*").expect("valid regex"));
static NMU_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bnmu\b").expect("valid regex"));
static NON_MAINTAINER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)non-maintainer upload").expect("valid regex"));
static ACK_NMU: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:ackno|\back\b|confir|incorporat).*(?:\bnmu\b|non-maintainer)")
        .expect("valid regex")
});
static LOCAL_PACKAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\blocal\s+package\b").expect("valid regex"));
static ORPHAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)orphan").expect("valid regex"));
static QA_UPLOAD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)qa (?:group )?upload").expect("valid regex"));
static TEAM_UPLOAD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)team upload").expect("valid regex"));
static VERSION_LOCAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)local").expect("valid regex"));

pub(super) struct NmuFacts {
    pub line: u32,
    pub version: String,
    pub no_nmu: bool,
    pub incorrect_version: bool,
    pub no_qa: bool,
    pub qa_incorrect_version: bool,
}

pub(super) fn nmu_facts(ctx: &SourceContext<'_>) -> Option<NmuFacts> {
    let entry = ctx.debian_changelog()?.entries().first()?;
    let maintainer = canonicalize(&control_field(ctx, "Maintainer"));
    let mut uploader = canonicalize(entry.maintainer());
    let trimmed = uploader.trim();
    if trimmed.len() != uploader.len() {
        uploader = trimmed.to_string();
    }

    let version = if entry.version().is_empty() {
        "0-1".to_string()
    } else {
        entry.version().to_string()
    };
    let nmuness = version_nmuness(&version);
    let version_local = VERSION_LOCAL.is_match(&version);
    let upload_is_backport = BACKPORT.is_match(&version);
    let upload_is_stable_update = STABLE_UPDATE.is_match(&version);

    let mut upload_is_nmu = uploader != maintainer;
    let listed = split_uploaders(&control_field(ctx, "Uploaders"));
    if listed.iter().any(|person| canonicalize(person) == uploader) {
        upload_is_nmu = false;
    }
    if uploader.is_empty() {
        upload_is_nmu = false;
    }

    let first = first_star_line(entry.changes());
    let mentions_nmu = first.is_some_and(mentions_nmu);
    let mentions_local = first.is_some_and(|line| LOCAL_PACKAGE.is_match(line));
    let mentions_qa = first.is_some_and(|line| ORPHAN.is_match(line) || QA_UPLOAD.is_match(line));
    let mentions_team = first.is_some_and(|line| TEAM_UPLOAD.is_match(line));

    let mut facts = NmuFacts {
        line: entry.line(),
        version,
        no_nmu: false,
        incorrect_version: false,
        no_qa: false,
        qa_incorrect_version: false,
    };
    if maintainer.contains("packages@qa.debian.org") {
        facts.no_qa = !mentions_qa;
        facts.qa_incorrect_version = nmuness == 1;
    } else if !mentions_team && !mentions_local && !version_local {
        facts.no_nmu = !mentions_nmu && upload_is_nmu;
        facts.incorrect_version =
            upload_is_nmu && nmuness != 1 && !upload_is_stable_update && !upload_is_backport;
    }
    Some(facts)
}

/// Changelog line when Maintainer is the QA group and `Uploaders` is declared.
///
/// The domain is lowercased the way `canonicalize` does. A missing changelog
/// entry does not emit: the Check returns before it reads Maintainer.
pub(super) fn uploaders_in_orphan_line(ctx: &SourceContext<'_>) -> Option<u32> {
    let entry = ctx.debian_changelog()?.entries().first()?;
    let maintainer = canonicalize(&control_field(ctx, "Maintainer"));
    if !maintainer.contains("packages@qa.debian.org") {
        return None;
    }
    if !ctx
        .debian_control()
        .is_some_and(|control| control.source().declares("Uploaders"))
    {
        return None;
    }
    Some(entry.line())
}

fn control_field(ctx: &SourceContext<'_>, name: &str) -> String {
    ctx.debian_control()
        .and_then(|control| control.source().unfolded_value(name))
        .unwrap_or_default()
}

/// Lowercase the domain of the first `<local@domain>`.
fn canonicalize(maintainer: &str) -> String {
    DOMAIN
        .replace(maintainer, |caps: &regex::Captures<'_>| {
            format!("<{}{}>", &caps[1], caps[2].to_lowercase())
        })
        .into_owned()
}

/// `split />\K\s*,\s*/` keeps the `>` on the address before the comma.
fn split_uploaders(uploaders: &str) -> Vec<String> {
    if uploaders.is_empty() {
        return Vec::new();
    }
    let pieces: Vec<&str> = UPLOADER_SPLIT.split(uploaders).collect();
    pieces
        .iter()
        .enumerate()
        .map(|(index, piece)| {
            if index + 1 < pieces.len() {
                format!("{piece}>")
            } else {
                (*piece).to_string()
            }
        })
        .collect()
}

fn version_nmuness(version: &str) -> u8 {
    let mut nmuness = 0;
    if let Some(caps) = REVISION_NMU.captures(version) {
        if caps.get(1).is_some() {
            nmuness = 1;
        }
        if caps.get(2).is_some() {
            nmuness = 2;
        }
    }
    if PLUS_NMU.is_match(version) {
        nmuness = 1;
    }
    if PLUS_BIN.is_match(version) {
        nmuness = 2;
    }
    nmuness
}

fn mentions_nmu(line: &str) -> bool {
    let named = NMU_WORD.is_match(line)
        || NON_MAINTAINER.is_match(line)
        || line.contains("LowThresholdNMU");
    named && !ACK_NMU.is_match(line)
}

fn first_star_line(changes: &str) -> Option<&str> {
    let mut rest = changes;
    while let Some(newline) = rest.find('\n') {
        if rest[..newline].chars().all(char::is_whitespace) {
            rest = &rest[newline + 1..];
        } else {
            break;
        }
    }
    rest.lines().find(|line| line.trim_start().starts_with('*'))
}
