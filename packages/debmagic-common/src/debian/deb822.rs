//! Deb822 paragraphs as read-only field lists: values, the text as written,
//! and the line each field starts on.
//!
//! Parsing never fails: [`parse`] returns the paragraphs it found together
//! with the parser's errors, and each caller decides what an error means (a
//! missing Debian control, a runtime failure, or a tolerated file). PGP
//! clearsign armor is not handled here; strip it first.

use std::sync::LazyLock;

use deb822_lossless::{Deb822, Entry, ParseError};
use regex::Regex;

static FOLD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n\s*").expect("valid regex"));

/// The value on one line: surrounding space trimmed, each line break and
/// the indentation after it a single space (lintian's `unfolded_value`).
pub fn unfold(value: &str) -> String {
    FOLD.replace_all(value.trim(), " ").into_owned()
}

/// One Deb822 paragraph. Field names match case-insensitively; a repeated
/// name answers with its first occurrence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Paragraph {
    fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Field {
    name: String,
    /// The value as `deb822-lossless` reads it: continuation lines joined
    /// with `\n`, their indentation removed.
    value: String,
    /// The value as written; see [`Paragraph::untrimmed_value`]. `None` for
    /// paragraphs built from plain values.
    untrimmed: Option<String>,
    /// 1-based line of the field name. `None` for plain values.
    line: Option<u32>,
}

impl Paragraph {
    /// A paragraph from plain `(name, value)` pairs, without text as written
    /// or line numbers.
    pub fn from_values(
        fields: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        Self {
            fields: fields
                .into_iter()
                .map(|(name, value)| Field {
                    name: name.into(),
                    value: value.into(),
                    untrimmed: None,
                    line: None,
                })
                .collect(),
        }
    }

    /// The fields of a lossless paragraph, in file order. Entries without a
    /// field name are skipped.
    pub fn from_lossless(paragraph: &deb822_lossless::Paragraph) -> Self {
        Self {
            fields: paragraph.entries().filter_map(located_field).collect(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.field(name).map(|field| field.value.as_str())
    }

    pub fn declares(&self, name: &str) -> bool {
        self.field(name).is_some()
    }

    /// Field names in file order, as written.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.fields.iter().map(|field| field.name.as_str())
    }

    /// See [`unfold`].
    pub fn unfolded_value(&self, name: &str) -> Option<String> {
        self.get(name).map(unfold)
    }

    /// The value with continuation lines as written (a leading space and
    /// all), comment lines dropped: lintian's `untrimmed_value`. Falls back
    /// to [`Self::get`] for a paragraph built from plain values.
    pub fn untrimmed_value(&self, name: &str) -> Option<&str> {
        let field = self.field(name)?;
        Some(field.untrimmed.as_deref().unwrap_or(&field.value))
    }

    /// The 1-based line where the field's name is written. `None` when it is
    /// not declared or the paragraph was built from plain values.
    pub fn position(&self, name: &str) -> Option<u32> {
        self.field(name)?.line
    }

    fn field(&self, name: &str) -> Option<&Field> {
        self.fields
            .iter()
            .find(|field| field.name.eq_ignore_ascii_case(name))
    }
}

/// What [`parse`] found: the paragraphs and the parser's errors. The
/// paragraphs of a text with errors are the parser's best effort.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parsed {
    pub paragraphs: Vec<Paragraph>,
    pub errors: Vec<String>,
}

impl Parsed {
    /// The paragraphs, or the errors when there are any.
    pub fn into_result(self) -> Result<Vec<Paragraph>, ParseError> {
        if self.errors.is_empty() {
            Ok(self.paragraphs)
        } else {
            Err(ParseError(self.errors))
        }
    }
}

/// Every paragraph of a Deb822 text, with the parser's errors.
pub fn parse(text: &str) -> Parsed {
    let parse = Deb822::parse(text);
    let errors = parse.errors().to_vec();
    let paragraphs = parse
        .tree()
        .paragraphs()
        .map(|paragraph| Paragraph::from_lossless(&paragraph))
        .collect();
    Parsed { paragraphs, errors }
}

fn located_field(entry: Entry) -> Option<Field> {
    let line = u32::try_from(entry.line()).ok()?.checked_add(1);
    let name = entry.key()?;
    let untrimmed = untrimmed_from_text(&entry.to_string());
    Some(Field {
        name,
        value: entry.value(),
        untrimmed: Some(untrimmed),
        line,
    })
}

/// What follows `Name:` in a field's text: leading blanks of the first line
/// and the final line break dropped, comment lines removed.
fn untrimmed_from_text(text: &str) -> String {
    let after_colon = text.split_once(':').map_or(text, |(_, rest)| rest);
    let value = after_colon.trim_start_matches([' ', '\t']);
    let value = value.strip_suffix('\n').unwrap_or(value);
    value
        .split('\n')
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "Source: example\n\
Maintainer: Example <ex@example.com>\n\
Build-Depends: debhelper-compat (= 13),\n  libfoo-dev,\n# a comment\n  libbar-dev\n\
\n\
# comment before the stanza\n\
Package: example\n\
Description: example\n extra\n";

    #[test]
    fn parses_paragraphs_with_values_and_lines() {
        let parsed = parse(TEXT);
        assert!(parsed.errors.is_empty());
        let [source, binary] = parsed.paragraphs.as_slice() else {
            panic!("two paragraphs expected");
        };
        assert_eq!(source.get("source"), Some("example"));
        assert_eq!(source.position("Maintainer"), Some(2));
        assert_eq!(binary.position("Package"), Some(9));
        assert_eq!(binary.position("Description"), Some(10));
        assert_eq!(
            source.names().collect::<Vec<_>>(),
            ["Source", "Maintainer", "Build-Depends"]
        );
        assert!(!source.declares("Package"));
    }

    #[test]
    fn untrimmed_keeps_indentation_and_drops_comments() {
        let source = &parse(TEXT).paragraphs[0];
        assert_eq!(
            source.untrimmed_value("Build-Depends"),
            Some("debhelper-compat (= 13),\n  libfoo-dev,\n  libbar-dev")
        );
        assert_eq!(
            source.unfolded_value("Build-Depends").as_deref(),
            Some("debhelper-compat (= 13), libfoo-dev, libbar-dev")
        );
    }

    #[test]
    fn errors_are_data() {
        let parsed = parse("not a deb822 file\n: \n");
        assert!(!parsed.errors.is_empty());
        assert!(parsed.into_result().is_err());
        assert_eq!(parse("").into_result(), Ok(Vec::new()));
    }

    #[test]
    fn plain_values_have_no_lines_and_untrimmed_falls_back() {
        let paragraph = Paragraph::from_values([("Source", "example")]);
        assert_eq!(paragraph.position("Source"), None);
        assert_eq!(paragraph.untrimmed_value("source"), Some("example"));
    }
}
