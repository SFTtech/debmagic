//! `debian/upstream/metadata` (DEP-12), parsed once per Subject.
//!
//! Lintian's `debian/upstream/metadata` Check loads the file with YAML::XS and
//! looks at the document's top-level mapping. Here the file is read from the
//! packaging File tree and parsed with `yaml-rust2`, the maintained fork of
//! `yaml-rust` (libyaml's scanner and parser ported to Rust, so its error
//! texts and positions follow libyaml's). A parse error keeps its reason,
//! document number, line and column on the object, so the YAML Rules can
//! format them the way lintian does.

// The first Rules that read it land after this seam; the allow goes then.
#![allow(dead_code)]

use std::path::Path;

use std::str::Chars;

use yaml_rust2::parser::{Event, Parser};
use yaml_rust2::scanner::{ScanError, TScalarStyle};

/// Path of the metadata file in the packaging File tree.
pub(crate) const UPSTREAM_METADATA_PATH: &str = "debian/upstream/metadata";

/// What `debian/upstream/metadata` is, as the Check sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UpstreamMetadata {
    /// No entry at the path.
    Missing,
    /// An entry exists but is not an open-able file: a directory, a fifo or
    /// device, a dangling or escaping symlink, or a loop. Same test as
    /// lintian's `is_open_ok`.
    NotAFile,
    /// An open-able file that holds no YAML document, or only a null one.
    /// Lintian dies on this with a Perl message, which is not reproduced.
    Empty,
    /// The first document is valid YAML but not a mapping.
    NotMapping,
    /// The file is not valid YAML.
    Invalid(YamlError),
    /// The first document is a top-level mapping.
    Mapping(Mapping),
}

/// The top-level mapping of the metadata file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mapping {
    fields: Vec<(String, FieldValue)>,
}

/// A top-level value. Lintian's `ref` test separates scalars from
/// references, and `defined` separates null from the rest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FieldValue {
    /// YAML null, or a key with no value. Lintian's `defined` is false.
    Null,
    /// A scalar (string, number, boolean) with its text as written.
    Scalar(String),
    /// A sequence or a mapping. Aliases are not resolved and count here too.
    NonScalar,
}

/// A YAML syntax error, worded and positioned as YAML::XS reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct YamlError {
    /// libyaml's problem text, e.g. `did not find expected <document start>`.
    pub reason: String,
    /// One-based number of the document the error is in.
    pub document: usize,
    /// One-based line.
    pub line: usize,
    /// One-based column.
    pub column: usize,
}

impl std::fmt::Display for YamlError {
    /// The message lintian's `upstream-metadata-yaml-invalid` carries.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (at document {}, line {}, column {})",
            self.reason, self.document, self.line, self.column
        )
    }
}

impl UpstreamMetadata {
    /// Parse the bytes of `debian/upstream/metadata`.
    pub(crate) fn parse(bytes: &[u8]) -> Self {
        let text = match std::str::from_utf8(bytes) {
            Ok(text) => text,
            Err(error) => {
                let (line, column) = position(bytes, error.valid_up_to());
                return Self::Invalid(YamlError {
                    reason: "invalid UTF-8 byte sequence".to_owned(),
                    document: 1,
                    line,
                    column,
                });
            }
        };
        let mut events = Events::new(text);
        let mut first = None;
        loop {
            let outcome = match events.next() {
                Ok(Event::StreamEnd) => break,
                Ok(Event::DocumentStart) => events.document(),
                Ok(_) => continue,
                Err(error) => Err(error),
            };
            match outcome {
                Ok(root) => {
                    first.get_or_insert(root);
                }
                Err(error) => return Self::Invalid(events.error(&error)),
            }
        }
        match first {
            None | Some(Root::Empty) => Self::Empty,
            Some(Root::NotMapping) => Self::NotMapping,
            Some(Root::Mapping(fields)) => Self::Mapping(Mapping { fields }),
        }
    }

    /// Whether the entry exists but is not an open-able file.
    #[allow(dead_code)]
    pub(crate) fn is_not_a_file(&self) -> bool {
        matches!(self, Self::NotAFile)
    }
}

/// Kind of the first document's root node.
enum Root {
    Empty,
    NotMapping,
    Mapping(Vec<(String, FieldValue)>),
}

/// Parser events for a whole stream, with what the error message needs.
///
/// Built on yaml-rust2's event parser rather than its loader: the loader
/// rejects a repeated key, which YAML::XS accepts (the last value wins). Only
/// the first document's top-level mapping is kept; the rest are scanned so a
/// syntax error in them is still reported.
struct Events<'a> {
    parser: Parser<Chars<'a>>,
    documents: usize,
    after_end: bool,
}

impl<'a> Events<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            parser: Parser::new_from_str(text),
            documents: 0,
            after_end: false,
        }
    }

    fn next(&mut self) -> Result<Event, ScanError> {
        let (event, _) = self.parser.next_token()?;
        match event {
            Event::DocumentStart => {
                self.documents += 1;
                self.after_end = false;
            }
            Event::DocumentEnd => self.after_end = true,
            _ => {}
        }
        Ok(event)
    }

    /// One document, after its `DocumentStart` was read. Consumes its
    /// `DocumentEnd`.
    fn document(&mut self) -> Result<Root, ScanError> {
        let root = match self.next()? {
            Event::DocumentEnd => return Ok(Root::Empty),
            Event::Scalar(value, style, ..) => {
                if is_null(&value, style) {
                    Root::Empty
                } else {
                    Root::NotMapping
                }
            }
            Event::MappingStart(..) => Root::Mapping(self.mapping()?),
            event => {
                self.skip(event)?;
                Root::NotMapping
            }
        };
        loop {
            if matches!(self.next()?, Event::DocumentEnd) {
                return Ok(root);
            }
        }
    }

    /// The entries of a mapping, after its `MappingStart`. A repeated key
    /// keeps its first position and its last value.
    fn mapping(&mut self) -> Result<Vec<(String, FieldValue)>, ScanError> {
        let mut fields: Vec<(String, FieldValue)> = Vec::new();
        loop {
            match self.next()? {
                Event::MappingEnd => return Ok(fields),
                Event::Scalar(name, style, ..) => {
                    let name = if is_null(&name, style) {
                        String::new()
                    } else {
                        name
                    };
                    let value = self.value()?;
                    match fields.iter_mut().find(|(key, _)| *key == name) {
                        Some(entry) => entry.1 = value,
                        None => fields.push((name, value)),
                    }
                }
                key => {
                    // A non-scalar key has no name lintian would report.
                    self.skip(key)?;
                    self.value()?;
                }
            }
        }
    }

    /// The kind of the value that follows a key.
    fn value(&mut self) -> Result<FieldValue, ScanError> {
        match self.next()? {
            Event::Scalar(value, style, ..) if is_null(&value, style) => Ok(FieldValue::Null),
            Event::Scalar(text, ..) => Ok(FieldValue::Scalar(text)),
            Event::Alias(_) => Ok(FieldValue::NonScalar),
            Event::SequenceStart(..) | Event::MappingStart(..) => {
                self.skip_rest()?;
                Ok(FieldValue::NonScalar)
            }
            _ => Ok(FieldValue::Null),
        }
    }

    /// Skip the node that `event` starts, whose events are already read.
    fn skip(&mut self, event: Event) -> Result<(), ScanError> {
        match event {
            Event::SequenceStart(..) | Event::MappingStart(..) => self.skip_rest(),
            _ => Ok(()),
        }
    }

    /// Skip events up to the end of the collection whose start was read.
    fn skip_rest(&mut self) -> Result<(), ScanError> {
        let mut depth = 1usize;
        while depth > 0 {
            match self.next()? {
                Event::SequenceStart(..) | Event::MappingStart(..) => depth += 1,
                Event::SequenceEnd | Event::MappingEnd => depth -= 1,
                _ => {}
            }
        }
        Ok(())
    }

    /// The error in YAML::XS's wording and numbering.
    ///
    /// The document number is the count of documents started before the error.
    /// libyaml counts the next document as started once the previous one has
    /// ended, so an error between two documents is reported in the later one.
    fn error(&self, error: &ScanError) -> YamlError {
        let marker = error.marker();
        YamlError {
            reason: error.info().to_owned(),
            document: (self.documents + usize::from(self.after_end)).max(1),
            line: marker.line(),
            column: marker.col() + 1,
        }
    }
}

/// YAML 1.1 null as libyaml resolves a plain scalar: `~`, `null`, or empty.
fn is_null(value: &str, style: TScalarStyle) -> bool {
    style == TScalarStyle::Plain && matches!(value, "" | "~" | "null" | "Null" | "NULL")
}

impl Mapping {
    /// Top-level keys in document order. A repeated key keeps its last value.
    pub(crate) fn fields(&self) -> &[(String, FieldValue)] {
        &self.fields
    }

    /// The value of `key`, if the key is present.
    #[allow(dead_code)]
    pub(crate) fn get(&self, key: &str) -> Option<&FieldValue> {
        self.fields
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    /// Text of `key` when its value is a scalar (lintian's `$yaml->{key}` used
    /// as a string).
    pub(crate) fn text(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            FieldValue::Scalar(text) => Some(text),
            FieldValue::Null | FieldValue::NonScalar => None,
        }
    }

    /// Whether `key` is present with a value that is not null (lintian's
    /// `defined $yaml->{key}`).
    pub(crate) fn is_defined(&self, key: &str) -> bool {
        self.get(key)
            .is_some_and(|value| *value != FieldValue::Null)
    }
}

/// One-based line and column of byte `offset` in `bytes`.
fn position(bytes: &[u8], offset: usize) -> (usize, usize) {
    let before = &bytes[..offset.min(bytes.len())];
    let line = before.iter().filter(|byte| **byte == b'\n').count() + 1;
    let line_start = before
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index + 1);
    let column = String::from_utf8_lossy(&before[line_start..])
        .chars()
        .count()
        + 1;
    (line, column)
}

/// Read `debian/upstream/metadata` from a packaging File tree and parse it.
pub(crate) fn read(files: &crate::lint::file_tree::FileTree) -> UpstreamMetadata {
    use crate::lint::context::follow_symlinks;

    let path = Path::new(UPSTREAM_METADATA_PATH);
    let Some(entry) = files.get(path) else {
        return UpstreamMetadata::Missing;
    };
    let Some(target) = follow_symlinks(files, entry) else {
        return UpstreamMetadata::NotAFile;
    };
    match files.read(target) {
        Ok(bytes) => UpstreamMetadata::parse(&bytes),
        Err(_) => UpstreamMetadata::NotAFile,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(text: &str) -> Mapping {
        match UpstreamMetadata::parse(text.as_bytes()) {
            UpstreamMetadata::Mapping(mapping) => mapping,
            other => panic!("expected a mapping, got {other:?}"),
        }
    }

    fn invalid(text: &str) -> YamlError {
        match UpstreamMetadata::parse(text.as_bytes()) {
            UpstreamMetadata::Invalid(error) => error,
            other => panic!("expected a YAML error, got {other:?}"),
        }
    }

    #[test]
    fn mapping_keeps_keys_in_document_order_and_value_kinds() {
        let metadata =
            mapping("Name: x\nRepository: ~\nBug-Database:\nArchive: [a]\nContact:\n  - b\n");
        assert_eq!(
            metadata.fields(),
            [
                ("Name".to_owned(), FieldValue::Scalar("x".to_owned())),
                ("Repository".to_owned(), FieldValue::Null),
                ("Bug-Database".to_owned(), FieldValue::Null),
                ("Archive".to_owned(), FieldValue::NonScalar),
                ("Contact".to_owned(), FieldValue::NonScalar),
            ]
        );
    }

    #[test]
    fn text_is_only_for_scalars() {
        let metadata = mapping("Name: node-glob\nArchive: [a]\nRepository: ~\n");
        assert_eq!(metadata.text("Name"), Some("node-glob"));
        assert_eq!(metadata.text("Archive"), None);
        assert_eq!(metadata.text("Repository"), None);
        assert_eq!(metadata.text("Absent"), None);
    }

    #[test]
    fn defined_is_false_for_null_and_absent_keys() {
        let metadata = mapping("Repository: ~\nBug-Submit: https://example.com\n");
        assert!(!metadata.is_defined("Repository"));
        assert!(!metadata.is_defined("Repository-Browse"));
        assert!(metadata.is_defined("Bug-Submit"));
    }

    #[test]
    fn repeated_key_keeps_its_last_value_in_first_position() {
        let metadata = mapping("Name: x\nRepository: ~\nName: 1\nRepository: y\n");
        assert_eq!(
            metadata.fields(),
            [
                ("Name".to_owned(), FieldValue::Scalar("1".to_owned())),
                ("Repository".to_owned(), FieldValue::Scalar("y".to_owned())),
            ]
        );
    }

    #[test]
    fn non_string_keys_are_named_as_scalars() {
        let metadata = mapping("1: x\ntrue: y\n");
        assert_eq!(metadata.text("1"), Some("x"));
        assert_eq!(metadata.text("true"), Some("y"));
    }

    #[test]
    fn sequence_and_scalar_roots_are_not_mappings() {
        assert_eq!(
            UpstreamMetadata::parse(b"- Name: x\n"),
            UpstreamMetadata::NotMapping
        );
        assert_eq!(
            UpstreamMetadata::parse(b"plain\n"),
            UpstreamMetadata::NotMapping
        );
    }

    #[test]
    fn empty_and_null_documents_are_empty() {
        assert_eq!(UpstreamMetadata::parse(b""), UpstreamMetadata::Empty);
        assert_eq!(
            UpstreamMetadata::parse(b"# only a comment\n"),
            UpstreamMetadata::Empty
        );
        assert_eq!(UpstreamMetadata::parse(b"~\n"), UpstreamMetadata::Empty);
    }

    #[test]
    fn syntax_error_reports_document_line_and_column() {
        let error = invalid(" - Hi,\nThis is a completely invalid YAML file.\n");
        assert_eq!(error.reason, "did not find expected <document start>");
        assert_eq!((error.document, error.line, error.column), (2, 2, 1));
        assert_eq!(
            error.to_string(),
            "did not find expected <document start> (at document 2, line 2, column 1)"
        );
    }

    #[test]
    fn syntax_error_inside_the_first_document_is_document_one() {
        let error = invalid("Name: x\nBad: [unclosed\n");
        assert_eq!(error.document, 1);
        assert_eq!(error.line, 3);
    }

    #[test]
    fn invalid_utf8_is_reported_at_its_position() {
        let error = match UpstreamMetadata::parse(b"Name: x\nDesc: \xff\n") {
            UpstreamMetadata::Invalid(error) => error,
            other => panic!("expected an error, got {other:?}"),
        };
        assert_eq!((error.line, error.column), (2, 7));
    }
}
