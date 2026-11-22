//! Lintian's doc-base file walk (`check_doc_base_file` and friends).
//!
//! One pass per file collects every hit the `menus` Check raises from the
//! parse itself, the spelling of the title and abstract included. Each Rule
//! reports the hits of one [`Kind`]. `doc-base-file-references-missing-file`
//! needs the files of strongly related installables, so it is not one of
//! them: [`missing_references`] walks a package's doc-base files with the
//! [`ReferenceTargets`] of the Group Rule that asks.
//! The Tags with their own walk in `scan.rs` are not repeated here.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::Severity;
use crate::lint::spelling::{Exceptions, check_spelling, check_spelling_picky, group_exceptions};

static FIELD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\S+)\s*:\s*(.*)$").expect("valid regex"));
static CONTINUATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s+\S").expect("valid regex"));
static SEPARATOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\s*)$").expect("valid regex"));
static DOCUMENT_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9+.-]+$").expect("valid regex"));
static APPLICATIONS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^App(?:lication)?s/(.+)$").expect("valid regex"));
static SUBSECTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+)/[^/]+$").expect("valid regex"));
static TEMPLATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"manage\s+online\s+manuals\s.*Debian").expect("valid regex"));
static DOT_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s+)\.(\s*)$").expect("valid regex"));
static LEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\s+)(\S)").expect("valid regex"));

static SECTIONS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/doc-base/sections")).collect()
});

const KNOWN_FORMATS: &[&str] = &[
    "html",
    "text",
    "pdf",
    "postscript",
    "info",
    "dvi",
    "debiandoc-sgml",
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    AbstractIsTemplate,
    AbstractExtraLeadingWhitespace,
    DocumentEndsInWhitespace,
    DocumentNotInFirstLine,
    NoFormat,
    NoIndex,
    ReferencesWrongPath,
    ReferencesMissingFile,
    FileSeparatorExtraWhitespace,
    SyntaxError,
    UnknownFormat,
    IndexReferencesMultipleFiles,
    InvalidDocument,
    UnknownSection,
    UsesApplicationsSection,
    DuplicateField,
    DuplicateFormat,
    SpellingTitle,
    SpellingAbstract,
}

pub(super) struct Hit {
    kind: Kind,
    line: u32,
    extra: String,
}

/// Report every hit of `kind` from the doc-base files in the data files.
pub(super) fn report(ctx: &mut BinaryPackageContext<'_>, kind: Kind, warning: bool) {
    emit(ctx, kind, |extra| {
        if warning {
            Diagnostic::warn(extra)
        } else {
            Diagnostic::error(extra)
        }
    });
}

/// Report every spelling hit of `kind` (`SpellingTitle` or `SpellingAbstract`)
/// as a pedantic warning, with the file as Location and the field's line.
pub(super) fn report_spelling(ctx: &mut BinaryPackageContext<'_>, kind: Kind) {
    emit(ctx, kind, |extra| {
        Diagnostic::warn(extra).with_severity(Severity::Pedantic)
    });
}

fn emit(ctx: &mut BinaryPackageContext<'_>, kind: Kind, diagnostic: impl Fn(String) -> Diagnostic) {
    let package = ctx.package_name().to_string();
    let exceptions = group_exceptions(ctx);
    let found = {
        let Some(files) = ctx.data_files() else {
            return;
        };
        hits(files, &package, &exceptions)
    };
    for (path, hit) in found {
        if hit.kind != kind {
            continue;
        }
        ctx.diagnostic(diagnostic(hit.extra).with_location(Location {
            path,
            line: Some(hit.line),
            column: None,
        }));
    }
}

fn hits(files: &FileTree, package: &str, exceptions: &Exceptions) -> Vec<(PathBuf, Hit)> {
    let mut entries: Vec<_> = files
        .entries()
        .iter()
        .filter(|entry| {
            matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
                && is_doc_base_child(entry.path())
        })
        .collect();
    entries.sort_by(|left, right| left.path().cmp(right.path()));
    let mut all = Vec::new();
    for entry in entries {
        let Ok(bytes) = files.read(entry) else {
            continue;
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        for hit in Walk::new(package, exceptions).run(text) {
            all.push((entry.path().to_path_buf(), hit));
        }
    }
    all
}

fn is_doc_base_child(path: &Path) -> bool {
    let Some(name) = path
        .to_str()
        .and_then(|text| text.strip_prefix("usr/share/doc-base/"))
    else {
        return false;
    };
    !name.is_empty() && !name.contains('/')
}

/// Lines as Perl's `split(/\n/)`: trailing empty lines vanish.
fn perl_lines(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines
}

struct OpenField {
    name: String,
    line: u32,
    vals: Vec<String>,
}

struct Walk<'a> {
    package: &'a str,
    exceptions: &'a Exceptions,
    /// The files the doc-base entries may name; `None` leaves
    /// `ReferencesMissingFile` out.
    targets: Option<&'a ReferenceTargets>,
    /// Check the spelling of the title and abstract.
    spelling: bool,
    hits: Vec<Hit>,
    saw_fields: HashSet<String>,
    saw_formats: HashSet<String>,
    current_format: Option<String>,
}

impl<'a> Walk<'a> {
    fn new(package: &'a str, exceptions: &'a Exceptions) -> Self {
        Self {
            package,
            exceptions,
            targets: None,
            spelling: true,
            hits: Vec::new(),
            saw_fields: HashSet::new(),
            saw_formats: HashSet::new(),
            current_format: None,
        }
    }

    fn hit(&mut self, kind: Kind, line: u32, extra: impl Into<String>) {
        self.hits.push(Hit {
            kind,
            line,
            // Lintian's output trims the extra.
            extra: extra.into().trim().to_string(),
        });
    }

    fn run(mut self, text: &str) -> Vec<Hit> {
        let mut field: Option<OpenField> = None;
        for (index, string) in perl_lines(text).into_iter().enumerate() {
            let position = u32::try_from(index + 1).unwrap_or(u32::MAX);
            if let Some(caps) = FIELD.captures(string) {
                if let Some(open) = field.take() {
                    self.check_field(&open);
                }
                field = Some(OpenField {
                    name: caps[1].to_string(),
                    line: position,
                    vals: vec![caps[2].to_string()],
                });
            } else if let Some(open) = field.as_mut()
                && CONTINUATION.is_match(string)
            {
                open.vals.push(string.to_string());
                open.line = position;
            } else if let Some(caps) = SEPARATOR.captures(string) {
                if !caps[1].is_empty() {
                    self.hit(Kind::FileSeparatorExtraWhitespace, position, "");
                }
                let Some(open) = field.take() else {
                    continue;
                };
                self.check_field(&open);
                self.check_section(open.line + 1);
                self.saw_fields.clear();
            } else {
                self.hit(Kind::SyntaxError, position, "");
            }
        }
        if let Some(open) = field {
            self.check_field(&open);
            self.check_section(open.line);
        }
        self.hits
    }

    fn check_field(&mut self, open: &OpenField) {
        let name = open.name.as_str();
        if self.saw_fields.contains(name) {
            self.hit(Kind::DuplicateField, open.line, name);
        }
        self.saw_fields.insert(name.to_string());
        match name {
            "Index" | "Files" => self.check_files(open),
            "Format" => self.check_format(open),
            "Document" => self.check_document(open),
            "Section" => self.check_section_field(open),
            "Title" => self.check_spelling_of(open, Kind::SpellingTitle),
            "Abstract" => self.check_abstract(open),
            _ => {}
        }
    }

    fn check_files(&mut self, open: &OpenField) {
        let files: Vec<&str> = open
            .vals
            .iter()
            .flat_map(|value| value.split_whitespace())
            .collect();
        if open.name == "Index" && files.len() > 1 {
            self.hit(Kind::IndexReferencesMultipleFiles, open.line, "");
        }
        for file in &files {
            if file.starts_with("/usr/share/doc/") || file.starts_with("/usr/share/info/") {
                continue;
            }
            self.hit(Kind::ReferencesWrongPath, open.line, *file);
        }
        if let Some(targets) = self.targets {
            let is_index = open.name == "Index";
            for file in &files {
                if !targets.contains(file, is_index) {
                    self.hit(Kind::ReferencesMissingFile, open.line, *file);
                }
            }
        }
    }

    fn check_format(&mut self, open: &OpenField) {
        let joined = open.vals.join(" ");
        let format = joined.trim().to_lowercase();
        if !KNOWN_FORMATS.contains(&format.as_str()) {
            self.hit(Kind::UnknownFormat, open.line, format.clone());
        }
        if self.saw_formats.contains(&format) {
            self.hit(Kind::DuplicateFormat, open.line, format.clone());
        }
        self.saw_formats.insert(format.clone());
        self.current_format = Some(format);
    }

    fn check_document(&mut self, open: &OpenField) {
        let joined = open.vals.join(" ");
        if !DOCUMENT_NAME.is_match(&joined) {
            self.hit(Kind::InvalidDocument, open.line, joined.clone());
        }
        if joined.ends_with([' ', '\t']) {
            self.hit(Kind::DocumentEndsInWhitespace, open.line, "");
        }
        if open.line != 1 {
            self.hit(Kind::DocumentNotInFirstLine, open.line, "");
        }
    }

    fn check_section_field(&mut self, open: &OpenField) {
        let joined = open.vals.join(" ");
        if SECTIONS.contains(joined.as_str()) {
            return;
        }
        if let Some(caps) = APPLICATIONS.captures(&joined)
            && SECTIONS.contains(&caps[1])
        {
            self.hit(Kind::UsesApplicationsSection, open.line, joined);
        } else if let Some(caps) = SUBSECTION.captures(&joined)
            && SECTIONS.contains(&caps[1])
        {
            // A new subsection of a known section.
        } else {
            self.hit(Kind::UnknownSection, open.line, joined);
        }
    }

    fn check_abstract(&mut self, open: &OpenField) {
        let mut leadsp: Option<String> = None;
        let mut charafter: Option<char> = None;
        let mut leadsp_ok = true;
        for value in open.vals.iter().skip(1) {
            if TEMPLATE.is_match(value) {
                if self.package != "doc-base" {
                    self.hit(Kind::AbstractIsTemplate, open.line, "");
                }
                continue;
            }
            if let Some(caps) = DOT_LINE.captures(value)
                && (&caps[1] != " " || !caps[2].is_empty())
            {
                // `doc-base-abstract-field-separator-extra-whitespace` is
                // reported by its own walk.
                continue;
            }
            let Some(caps) = LEADING.captures(value) else {
                continue;
            };
            let spaces = caps[1].to_string();
            let after = caps[2].chars().next();
            if leadsp.as_deref().is_none_or(str::is_empty) {
                leadsp_ok = spaces == " ";
                leadsp = Some(spaces);
                charafter = after;
            } else if !leadsp_ok {
                if truthy(charafter) && charafter != after {
                    charafter = None;
                }
                if leadsp.as_deref() != Some(spaces.as_str()) || truthy(charafter) {
                    leadsp_ok = true;
                }
            }
        }
        if !leadsp_ok {
            self.hit(Kind::AbstractExtraLeadingWhitespace, open.line, "");
        }
        self.check_spelling_of(open, Kind::SpellingAbstract);
    }

    /// `check_spelling` and `check_spelling_picky` over the field's values
    /// joined by spaces, each finding a hit of `kind` on the field's line.
    fn check_spelling_of(&mut self, open: &OpenField, kind: Kind) {
        if !self.spelling {
            return;
        }
        let text = open.vals.join(" ");
        let findings = check_spelling(&text, self.exceptions, true)
            .into_iter()
            .chain(check_spelling_picky(&text));
        for (found, correction) in findings {
            self.hit(kind, open.line, format!("{found} {correction}"));
        }
    }

    fn check_section(&mut self, line: u32) {
        let files = self.saw_fields.contains("Files") || self.saw_fields.contains("Index");
        let format = self.saw_fields.contains("Format");
        if files && !format {
            self.hit(Kind::NoFormat, line, "");
        }
        if format
            && let Some(current) = self.current_format.as_deref()
            && matches!(current, "html" | "info")
            && !self.saw_fields.contains("Index")
        {
            self.hit(Kind::NoIndex, line, "");
        }
    }
}

/// The doc-base files of one package, each with the hits of kind
/// `ReferencesMissingFile`: path, line and the entry that is not in `targets`.
/// Like lintian, an executable file is not read (another Tag reports it).
pub(super) fn missing_references(
    files: &FileTree,
    targets: &ReferenceTargets,
) -> Vec<(PathBuf, u32, String)> {
    let mut entries: Vec<_> = files
        .entries()
        .iter()
        .filter(|entry| {
            matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
                && !entry.is_executable_file()
                && is_doc_base_child(entry.path())
        })
        .collect();
    entries.sort_by(|left, right| left.path().cmp(right.path()));
    let none = Exceptions::new();
    let mut found = Vec::new();
    for entry in entries {
        let Ok(bytes) = files.read(entry) else {
            continue;
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        let mut walk = Walk::new("", &none);
        walk.targets = Some(targets);
        walk.spelling = false;
        for hit in walk.run(text) {
            if hit.kind == Kind::ReferencesMissingFile {
                found.push((entry.path().to_path_buf(), hit.line, hit.extra));
            }
        }
    }
    found
}

/// lintian's `$all_files` and `$all_links` of a package and the installables
/// it depends on: every path as an absolute name (a directory with a trailing
/// `/`), and where each symlink or hardlink leads.
#[derive(Default)]
pub(super) struct ReferenceTargets {
    files: HashSet<String>,
    links: HashMap<String, String>,
}

static DUPLICATE_SLASHES: LazyLock<Regex> = LazyLock::new(|| Regex::new("/+").expect("regex"));
static DOT_SEGMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/+\./+").expect("regex"));
static LEADING_DOTDOT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^/+\.\./+").expect("regex"));
static LAST_COMPONENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/[^/]*$").expect("regex"));
static FIRST_COMPONENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(/[^/]*)").expect("regex"));

impl ReferenceTargets {
    /// Add every Entry of a package's Data files (lintian's
    /// `add_file_link_info`).
    pub(super) fn add_tree(&mut self, tree: &FileTree) {
        for entry in tree.entries() {
            let mut file = format!("/{}", entry.path().to_string_lossy());
            if entry.kind() == EntryKind::Directory {
                file.push('/');
            }
            let file = DUPLICATE_SLASHES.replace_all(&file, "/").into_owned();
            self.files.insert(file.clone());
            let (link, is_hardlink) = match entry.kind() {
                EntryKind::Symlink => (entry.link_target(), false),
                EntryKind::Hardlink => (entry.hardlink_target(), true),
                _ => (None, false),
            };
            let Some(link) = link.map(|link| link.to_string_lossy().into_owned()) else {
                continue;
            };
            if link.is_empty() {
                continue;
            }
            let link = if link.starts_with('/') {
                link
            } else if is_hardlink {
                format!("/{link}")
            } else {
                let link = DOT_SEGMENT
                    .replace_all(&format!("/./{link}"), "/")
                    .into_owned();
                let mut link = link;
                let mut up = 1;
                while let Some(found) = LEADING_DOTDOT.find(&link) {
                    link = format!("/{}", &link[found.end()..]);
                    up += 1;
                }
                let mut base = file.clone();
                for _ in 0..up {
                    base = LAST_COMPONENT.replace(&base, "").into_owned();
                }
                format!("{base}{link}")
            };
            if link != file {
                self.links.insert(file, link);
            }
        }
    }

    /// lintian's `delink`: the name with every symlink on the way replaced.
    fn delink(&self, file: &str) -> String {
        let file = DUPLICATE_SLASHES.replace_all(file, "/").into_owned();
        if self.links.is_empty() {
            return file;
        }
        let mut first = String::new();
        let mut rest = file;
        // lintian's own loop detection never triggers; stop after many hops.
        for _ in 0..10_000 {
            let Some(captures) = FIRST_COMPONENT.captures(&rest) else {
                break;
            };
            let component = captures[1].to_string();
            rest = rest[component.len()..].to_string();
            first.push_str(&component);
            if let Some(target) = self.links.get(&first) {
                rest = format!("{target}{rest}");
                first.clear();
            }
        }
        if first.is_empty() { rest } else { first }
    }

    /// The entry of a doc-base `Index` or `Files` field is in the package or
    /// its dependencies. A wildcard in `Files` is a pattern over a file name;
    /// one with a character class is not checked. In `Index` a wildcard is
    /// taken literally.
    fn contains(&self, file: &str, is_index: bool) -> bool {
        let real = self.delink(file);
        if real.contains(['*', '?']) {
            if is_index {
                return self.files.contains(&real);
            }
            if real.contains('[') {
                return true;
            }
            let mut pattern = String::from("^");
            for ch in real.chars() {
                match ch {
                    '*' => pattern.push_str("[^/]*"),
                    '?' => pattern.push_str("[^/]"),
                    other => pattern.push_str(&regex::escape(&other.to_string())),
                }
            }
            pattern.push_str("/?$");
            let Ok(pattern) = Regex::new(&pattern) else {
                return true;
            };
            return self.files.iter().any(|name| pattern.is_match(name));
        }
        self.files.contains(&real) || self.files.contains(&format!("{real}/"))
    }
}

/// Perl truthiness of a single character: `"0"` is false.
fn truthy(ch: Option<char>) -> bool {
    ch.is_some_and(|ch| ch != '0')
}
