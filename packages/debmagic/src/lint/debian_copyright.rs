use std::sync::LazyLock;

use regex::Regex;

/// Last `0+svn~N` revision of the DEP-5 draft before the overhaul; lintian
/// stops looking at anything older (`$LAST_DEP5_OVERHAUL`).
const LAST_DEP5_OVERHAUL_REVISION: u64 = 148;

/// Parsed `debian/copyright` (or `debian/<installable>.copyright`).
///
/// Mirrors the part of `Lintian::Check::Debian::Copyright::Dep5` that decides
/// whether the file is machine-readable and reads its paragraphs. It is not
/// Debian control: a file that is not copyright-format 1.0 is a plain text
/// copyright, and syntax errors stay on the value.
///
/// PGP armour is not supported; a signed copyright file is a syntax error here
/// (lintian reads through the armour).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebianCopyright {
    /// No machine-readable `Format` marker (lintian's `no-dep5-copyright`).
    NotMachineReadable,
    /// A marker is present but the `Format` field is missing or names no
    /// known version (boilerplate, unknown URI).
    UnknownFormat,
    /// A known DEP-5 draft older than the overhaul (wiki, early `0+svn`).
    Outdated,
    /// Copyright-format 1.0 or a post-overhaul draft. Paragraphs are `Err`
    /// with lintian's message on a Deb822 syntax error.
    MachineReadable {
        paragraphs: Result<Vec<CopyrightParagraph>, String>,
    },
}

/// One Deb822 paragraph of a machine-readable copyright, with line numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyrightParagraph {
    start_line: u32,
    fields: Vec<CopyrightField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CopyrightField {
    name: String,
    /// Verbatim: continuation lines joined with `\n`, each trimmed on the right.
    value: String,
    line: u32,
}

impl DebianCopyright {
    pub fn parse(text: &str) -> Self {
        if !MACHINE_READABLE_MARKER.is_match(text) {
            return Self::NotMachineReadable;
        }
        let Some(uri) = format_uri(text) else {
            return Self::UnknownFormat;
        };
        match find_dep5_version(&uri) {
            None => Self::UnknownFormat,
            Some(version) if version.is_before_overhaul() => Self::Outdated,
            Some(_) => Self::MachineReadable {
                paragraphs: parse_paragraphs(text),
            },
        }
    }

    /// Paragraphs of a machine-readable file; `None` for every other format
    /// and for a syntax error.
    pub fn paragraphs(&self) -> Option<&[CopyrightParagraph]> {
        match self {
            Self::MachineReadable {
                paragraphs: Ok(paragraphs),
            } => Some(paragraphs),
            _ => None,
        }
    }
}

impl CopyrightParagraph {
    /// Line of the paragraph's first field.
    #[allow(dead_code)]
    pub fn start_line(&self) -> u32 {
        self.start_line
    }

    pub fn declares(&self, name: &str) -> bool {
        self.field(name).is_some()
    }

    /// Line where the field starts, or `None` when it is not declared.
    pub fn position(&self, name: &str) -> Option<u32> {
        self.field(name).map(|field| field.line)
    }

    /// Field value with both ends trimmed; empty when not declared.
    pub fn value(&self, name: &str) -> &str {
        self.field(name).map_or("", |field| field.value.trim())
    }

    /// Whitespace-separated words of the field value (lintian's `trimmed_list`).
    pub fn trimmed_list(&self, name: &str) -> Vec<&str> {
        self.field(name)
            .map(|field| field.value.split_whitespace().collect())
            .unwrap_or_default()
    }

    fn field(&self, name: &str) -> Option<&CopyrightField> {
        self.fields
            .iter()
            .find(|field| field.name.eq_ignore_ascii_case(name))
    }
}

// `\bDEP ?5\b` is written under `/x` in lintian, so the space is not
// significant there: the optional character is the `P`.
static MACHINE_READABLE_MARKER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:\A|\n)(?i:format(?:[:]|[-\s]spec))(?:.|\n\s+)*(?:/dep[5s]?\b|\bDEP?5\b|[Mm]achine-readable\s(?:license|copyright)|/copyright-format/|CopyrightFormat|VERSIONED_FORMAT_URL)",
    )
    .expect("valid regex")
});
static FORMAT_FIELD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?mi)^Format(?:-Specification)?:\s*(\S+)\s*$").expect("valid regex")
});
static COMMENT_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^#.*").expect("valid regex"));
static TRAILING_BLANKS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)[ \t]+$").expect("valid regex"));
static FOLDED_NEWLINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\n?[ \t]+").expect("valid regex"));
static FRAGMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([^#\s]+)#").expect("valid regex"));

/// The Format URI of the first paragraph, as lintian extracts it.
fn format_uri(text: &str) -> Option<String> {
    let mut first = COMMENT_LINE.replace_all(text, "").into_owned();
    first = TRAILING_BLANKS.replace_all(&first, "").into_owned();
    first = first.trim_start_matches('\n').to_string();
    if let Some(end) = first.find("\n\n") {
        first.truncate(end);
        first.push('\n');
    }
    let first = FOLDED_NEWLINE.replace_all(&first, " ");
    let uri = FORMAT_FIELD.captures(&first)?.get(1)?.as_str().to_string();
    Some(FRAGMENT.replace(&uri, "$1").into_owned())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dep5Version {
    /// `0~wiki[~rev]`
    Wiki,
    /// `0+svn[~rev]`
    Svn(Option<u64>),
    /// `N.M` from the copyright-format URI.
    Release,
}

impl Dep5Version {
    fn is_before_overhaul(self) -> bool {
        match self {
            Self::Wiki => true,
            Self::Svn(Some(revision)) => revision < LAST_DEP5_OVERHAUL_REVISION,
            Self::Svn(None) | Self::Release => false,
        }
    }
}

static BOILERPLATE_URI: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:rev=REVISION|VERSIONED_FORMAT_URL)\b").expect("regex"));
static WIKI_URI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"https?://wiki\.debian\.org/Proposals/CopyrightFormat\b").expect("regex")
});
static DEP_SITE_URI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^https?://dep(-team\.pages)?\.debian\.net/deps/dep5/?$").expect("regex")
});
static SVN_WSVN_URI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\Ahttps?://svn\.debian\.org/wsvn/dep/web/deps/dep5\.mdwn\b").expect("regex")
});
static SVN_WSVN_REV: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\?(?:\S+[&;])?rev=(\d+)(?:[&;]\S+)?$").expect("regex"));
static SVN_VIEWVC_URI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\Ahttps?://(?:svn|anonscm)\.debian\.org/viewvc/dep/web/deps/dep5\.mdwn\b")
        .expect("regex")
});
static SVN_VIEWVC_REV: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\A\?(?:\S+[&;])?(?:pathrev|revision|rev)=(\d+)(?:[&;]\S+)?\z").expect("regex")
});
static RELEASE_URI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\Ahttps?://www\.debian\.org/doc/(?:packaging-manuals/)?copyright-format/\d+\.\d+/?\z",
    )
    .expect("regex")
});

/// Lintian's `find_dep5_version`; `None` where it returns `undef`.
fn find_dep5_version(uri: &str) -> Option<Dep5Version> {
    if BOILERPLATE_URI.is_match(uri) {
        return None;
    }
    if WIKI_URI.is_match(uri) {
        return Some(Dep5Version::Wiki);
    }
    if DEP_SITE_URI.is_match(uri) {
        return Some(Dep5Version::Svn(None));
    }
    for (prefix, rev) in [
        (&*SVN_WSVN_URI, &*SVN_WSVN_REV),
        (&*SVN_VIEWVC_URI, &*SVN_VIEWVC_REV),
    ] {
        if let Some(found) = prefix.find(uri) {
            let revision = rev
                .captures(&uri[found.end()..])
                .map(|captures| captures[1].parse().unwrap_or(u64::MAX));
            return Some(Dep5Version::Svn(revision));
        }
    }
    RELEASE_URI.is_match(uri).then_some(Dep5Version::Release)
}

static FIELD_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([^: \t]+):\s*(.*)$").expect("valid regex"));
static CONTINUATION_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[ \t].*\S.*$").expect("valid regex"));

/// Lintian's `visit_dpkg_paragraph_string` with no flags, minus PGP handling.
fn parse_paragraphs(text: &str) -> Result<Vec<CopyrightParagraph>, String> {
    let mut paragraphs = Vec::new();
    let mut open: Option<CopyrightParagraph> = None;
    for (index, line) in text.split('\n').enumerate() {
        let position = u32::try_from(index + 1).unwrap_or(u32::MAX);
        if line.starts_with('#') {
            continue;
        }
        if line.trim().is_empty() {
            paragraphs.extend(open.take());
            continue;
        }
        if line.starts_with("-----BEGIN PGP ") || line.starts_with("-----END PGP ") {
            return Err(format!("PGP armour is not supported (line {position})."));
        }
        if let Some(captures) = FIELD_LINE.captures(line) {
            let name = captures[1].to_string();
            let value = captures[2].trim_end().to_string();
            let paragraph = open.get_or_insert_with(|| CopyrightParagraph {
                start_line: position,
                fields: Vec::new(),
            });
            if paragraph.fields.iter().any(|field| field.name == name) {
                return Err(format!("Duplicate field {name} (line {position})."));
            }
            paragraph.fields.push(CopyrightField {
                name,
                value,
                line: position,
            });
        } else if CONTINUATION_LINE.is_match(line) {
            let Some(field) = open
                .as_mut()
                .and_then(|paragraph| paragraph.fields.last_mut())
            else {
                return Err(format!(
                    "Continuation line not in paragraph (line {position}). Missing a dot on the previous line?"
                ));
            };
            field.value.push('\n');
            field.value.push_str(line.trim_end());
        } else {
            return Err(format!("Cannot parse line {position}: {line}"));
        }
    }
    paragraphs.extend(open);
    Ok(paragraphs)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORMAT: &str =
        "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n";

    fn paragraphs(text: &str) -> Vec<CopyrightParagraph> {
        DebianCopyright::parse(text)
            .paragraphs()
            .expect("machine-readable")
            .to_vec()
    }

    #[test]
    fn plain_text_is_not_machine_readable() {
        assert_eq!(
            DebianCopyright::parse("Copyright 2020 Someone\nLicense: GPL-2\n"),
            DebianCopyright::NotMachineReadable
        );
    }

    #[test]
    fn machine_readable_has_paragraphs_with_lines() {
        let text = format!("{FORMAT}\n# comment\nFiles: a b\n  c\nCopyright: me\nLicense: MIT\n");
        let paragraphs = paragraphs(&text);
        assert_eq!(paragraphs.len(), 2);
        assert_eq!(paragraphs[0].start_line(), 1);
        assert_eq!(paragraphs[1].start_line(), 4);
        assert_eq!(paragraphs[1].position("files"), Some(4));
        assert_eq!(paragraphs[1].position("License"), Some(7));
        assert_eq!(paragraphs[1].position("Comment"), None);
        assert_eq!(paragraphs[1].trimmed_list("Files"), ["a", "b", "c"]);
        assert!(paragraphs[1].declares("copyright"));
    }

    #[test]
    fn empty_field_value_continues_on_next_line() {
        let text = format!("{FORMAT}\nFiles:\n one\n two \nCopyright: x\nLicense: y\n");
        assert_eq!(paragraphs(&text)[1].trimmed_list("Files"), ["one", "two"]);
    }

    #[test]
    fn syntax_errors_stay_on_the_value() {
        let duplicate = DebianCopyright::parse(&format!("{FORMAT}\nFiles: a\nFiles: b\n"));
        assert_eq!(
            duplicate,
            DebianCopyright::MachineReadable {
                paragraphs: Err("Duplicate field Files (line 4).".to_string())
            }
        );
        assert_eq!(duplicate.paragraphs(), None);
        let orphan = DebianCopyright::parse(&format!("{FORMAT}\n continuation\n"));
        assert!(matches!(
            orphan,
            DebianCopyright::MachineReadable { paragraphs: Err(_) }
        ));
        let junk = DebianCopyright::parse(&format!("{FORMAT}\nnot a field\n"));
        assert!(matches!(
            junk,
            DebianCopyright::MachineReadable { paragraphs: Err(message) }
                if message == "Cannot parse line 3: not a field"
        ));
    }

    #[test]
    fn format_versions() {
        let parse = |uri: &str| DebianCopyright::parse(&format!("Format: {uri}\n\nFiles: *\n"));
        let readable = |copyright: DebianCopyright| copyright.paragraphs().is_some();
        assert!(readable(parse(
            "https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/"
        )));
        assert!(readable(parse(
            "http://www.debian.org/doc/packaging-manuals/copyright-format/1.0"
        )));
        assert!(readable(parse(
            "https://dep-team.pages.debian.net/deps/dep5/"
        )));
        assert!(readable(parse(
            "http://svn.debian.org/wsvn/dep/web/deps/dep5.mdwn?op=file&rev=174"
        )));
        assert_eq!(
            parse("http://svn.debian.org/wsvn/dep/web/deps/dep5.mdwn?op=file&rev=147"),
            DebianCopyright::Outdated
        );
        assert_eq!(
            parse("http://wiki.debian.org/Proposals/CopyrightFormat?action=recall&rev=7"),
            DebianCopyright::Outdated
        );
        assert_eq!(
            parse("http://dep.debian.net/deps/dep5/?rev=REVISION"),
            DebianCopyright::UnknownFormat
        );
        assert_eq!(
            parse("https://example.org/copyright-format/"),
            DebianCopyright::UnknownFormat
        );
    }

    #[test]
    fn marker_without_format_field_is_unknown() {
        assert_eq!(
            DebianCopyright::parse("Upstream-Name: x\nThis is machine-readable copyright\n"),
            DebianCopyright::NotMachineReadable
        );
        assert_eq!(
            DebianCopyright::parse("Format-Specification\n /copyright-format/\n"),
            DebianCopyright::UnknownFormat
        );
    }
}
