//! Plain-text scans of an installed copyright file.
//!
//! `check_names_texts` in `Lintian::Check::Debian::Copyright` parses the file
//! as Deb822. A syntax error checks the whole file; a successful parse checks
//! each `License` field that has a name line and a following body. That parse
//! is not [`crate::lint::debian_copyright::DebianCopyright`]: it does not look
//! at the Format URI.

use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::BinaryPackageContext;

use super::installed_copyright;

const GPL_LENGTH: usize = 12_000;

/// Which common-license Tag the `unless` / `if` / `elsif` chain selects.
///
/// GFDL is part of the chain and suppresses the later Tags; this Check does
/// not emit it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CommonLicenseGap {
    Lgpl,
    Gpl,
    Apache2,
}

pub(super) fn installed_text(ctx: &mut BinaryPackageContext<'_>) -> Option<String> {
    if ctx.is_udeb() {
        return None;
    }
    let package = ctx.package_name().to_string();
    let files = ctx.data_files()?;
    installed_copyright::text(files, &package)
}

pub(super) fn lacks_perl_license_pointer(contents: &str) -> bool {
    license_matches(contents, &PERL_NAME, perl_text)
}

pub(super) fn contains_dh_make_todo(contents: &str) -> bool {
    contents.contains(DH_MAKE_LOOK) || DH_MAKE_GPL.is_match(contents)
}

pub(super) fn contains_full_gpl(contents: &str) -> bool {
    if contents.chars().count() <= GPL_LENGTH {
        return false;
    }
    GPL_V2_TERMS.is_match(contents)
        || (GPL_V3_HEADER.is_match(contents) && GPL_V3_TERMS.is_match(contents))
}

pub(super) fn common_license_gap(contents: &str) -> Option<CommonLicenseGap> {
    if common_license_exempt(contents) {
        return None;
    }
    if license_matches(contents, &GFDL_NAME, |text| GFDL_TEXT.is_match(text)) {
        return None;
    }
    if license_matches(contents, &LGPL_NAME, |text| LGPL_TEXT.is_match(text)) {
        return Some(CommonLicenseGap::Lgpl);
    }
    if license_matches(contents, &GPL_NAME, |text| GPL_TEXT.is_match(text)) {
        return Some(CommonLicenseGap::Gpl);
    }
    if license_matches(contents, &APACHE_NAME, |text| APACHE_TEXT.is_match(text)) {
        return Some(CommonLicenseGap::Apache2);
    }
    None
}

fn license_matches(contents: &str, name: &Regex, text_matches: impl Fn(&str) -> bool) -> bool {
    match license_bodies(contents) {
        Err(()) => text_matches(contents),
        Ok(bodies) => bodies
            .iter()
            .any(|(license_name, text)| name.is_match(license_name) && text_matches(text)),
    }
}

fn perl_text(text: &str) -> bool {
    PERL_ITSELF.is_match(text) && !text.contains("usr/share/common-licenses/")
}

fn common_license_exempt(contents: &str) -> bool {
    const LITERALS: &[&str] = &[
        "/usr/share/common-licenses",
        "Zope Public License",
        "LICENSE AGREEMENT FOR PYTHON 1.6.1",
        "LaTeX Project Public License",
        "AFFERO GENERAL PUBLIC LICENSE",
        "CeCILL FREE SOFTWARE LICENSE AGREEMENT",
        "FREE SOFTWARE LICENSING AGREEMENT CeCILL",
        "CNRI OPEN SOURCE GPL-COMPATIBLE LICENSE AGREEMENT",
        "usr/share/common-licences",
        "usr/share/doc/copyright",
        "usr/doc/copyright",
    ];
    LITERALS.iter().any(|literal| contents.contains(literal))
        || EXEMPT.iter().any(|pattern| pattern.is_match(contents))
}

/// `Err` when Deb822 parsing fails (the whole file is then the text).
/// Blank license bodies are omitted, matching the `next` skips.
fn license_bodies(contents: &str) -> Result<Vec<(String, String)>, ()> {
    let mut licenses = Vec::new();
    for fields in parse_paragraphs(contents)? {
        let Some(value) = fields
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("License"))
            .map(|(_, value)| value.trim())
        else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        let Some(captures) = LICENSE_PARTS.captures(value) else {
            continue;
        };
        let text = captures[2].to_string();
        if text.chars().all(char::is_whitespace) {
            continue;
        }
        licenses.push((captures[1].to_string(), text));
    }
    Ok(licenses)
}

fn parse_paragraphs(contents: &str) -> Result<Vec<Vec<(String, String)>>, ()> {
    let mut paragraphs = Vec::new();
    let mut fields: Vec<(String, String)> = Vec::new();
    for line in contents.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.starts_with('#') {
            continue;
        }
        if line.trim().is_empty() {
            if !fields.is_empty() {
                paragraphs.push(std::mem::take(&mut fields));
            }
            continue;
        }
        if let Some(captures) = FIELD.captures(line) {
            let name = captures[1].to_string();
            if fields.iter().any(|(existing, _)| existing == &name) {
                return Err(());
            }
            fields.push((name, captures[2].trim_end().to_string()));
            continue;
        }
        if let Some(captures) = CONTINUATION.captures(line) {
            let Some((_, value)) = fields.last_mut() else {
                return Err(());
            };
            value.push('\n');
            value.push_str(captures[1].trim_end());
            continue;
        }
        return Err(());
    }
    if !fields.is_empty() {
        paragraphs.push(fields);
    }
    Ok(paragraphs)
}

const DH_MAKE_LOOK: &str = "# Please also look if there are files or directories which have a\n# different copyright/license attached and list them here.";

static DH_MAKE_GPL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        "# If you want to use GPL v2 or later for the /debian/* files use\n# the following clauses, or change it to suit. Delete these two lines",
    )
    .expect("valid regex")
});
static GPL_V2_TERMS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bGNU GENERAL PUBLIC LICENSE\s*TERMS AND CONDITIONS FOR COPYING,\s*DISTRIBUTION AND MODIFICATION\b")
        .expect("valid regex")
});
static GPL_V3_HEADER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bGNU GENERAL PUBLIC LICENSE\s*Version 3").expect("valid regex"));
static GPL_V3_TERMS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bTERMS AND CONDITIONS\s").expect("valid regex"));
static FIELD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([^: \t]+):\s*(.*)$").expect("valid regex"));
static CONTINUATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([ \t].*\S.*)$").expect("valid regex"));
static LICENSE_PARTS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)^\s*([^\r\n]+)\r?\n(.*)\z").expect("valid regex"));
static PERL_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:perl|artistic)\b").expect("valid regex"));
static PERL_ITSELF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:under )?(?:the )?(?:same )?(?:terms )?as Perl itself\b")
        .expect("valid regex")
});
static GFDL_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:GFDL|gnu[-_]free[-_]documentation[-_]license)\b").expect("valid regex")
});
static GFDL_TEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i:GNU Free Documentation License)|\bGFDL\b").expect("valid regex")
});
static LGPL_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:LGPL|gnu[-_](?:lesser|library)[-_]general[-_]public[-_]license)\b")
        .expect("valid regex")
});
static LGPL_TEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i:GNU (?:Lesser|Library) General Public License)|\bLGPL\b").expect("valid regex")
});
static GPL_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:GPL|gnu[-_]general[-_]public[-_]license)\b").expect("valid regex")
});
static GPL_TEXT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i:GNU General Public License)|\bGPL\b").expect("valid regex"));
static APACHE_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bapache[-_]2").expect("valid regex"));
static APACHE_TEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i:\bApache License\s*,?\s*Version 2)|\bApache-2").expect("valid regex")
});
static EXEMPT: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?ms)(?:^From:.*^To:|^To:.*^From:).*(?:GNU General Public License|GPL)",
        r"GNU Free Documentation License[,\s]*Version 1\.1",
        r"compatible\s+with\s+(?:the\s+)?(?:GNU\s+)?GPL",
        r"(?:GNU\s+)?GPL\W+compatible",
        r"was\s+previously\s+(?:distributed\s+)?under\s+the\s+GNU",
        r"means\s+either\s+the\s+GNU\s+General\s+Public\s+License",
    ]
    .into_iter()
    .map(|pattern| Regex::new(pattern).expect("valid regex"))
    .collect()
});
