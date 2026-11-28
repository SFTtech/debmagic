//! Text helpers that follow Perl semantics, shared by the Rules that port
//! lintian's line and field walks.

use std::sync::OnceLock;

use regex::Regex;

/// A pattern with Unicode `\w` or `\d`, and its ASCII twin.
///
/// Building a Unicode `\w` or `\d` costs far more than the ASCII class, and most inputs are
/// ASCII. On an ASCII haystack, Unicode `\w` is `[0-9A-Za-z_]` and `\d` is `[0-9]`, so the twin
/// (the same pattern with `(?-u:\w)` / `(?-u:\d)`) matches the same text with the same groups.
/// [`AsciiTwin::regex_for`] therefore returns the twin for an ASCII haystack and builds the Unicode
/// regex only on the first non-ASCII one.
pub(crate) struct AsciiTwin {
    ascii_pattern: &'static str,
    unicode_pattern: &'static str,
    ascii: OnceLock<Regex>,
    unicode: OnceLock<Regex>,
}

impl AsciiTwin {
    pub(crate) const fn new(ascii_pattern: &'static str, unicode_pattern: &'static str) -> Self {
        Self {
            ascii_pattern,
            unicode_pattern,
            ascii: OnceLock::new(),
            unicode: OnceLock::new(),
        }
    }

    /// The regex to run on `haystack`, which must be the text it is then matched against.
    pub(crate) fn regex_for(&self, haystack: &str) -> &Regex {
        if haystack.is_ascii() {
            self.ascii
                .get_or_init(|| Regex::new(self.ascii_pattern).expect("valid regex"))
        } else {
            self.unicode
                .get_or_init(|| Regex::new(self.unicode_pattern).expect("valid regex"))
        }
    }
}

/// Perl `split /\n/, $text`: lines without their newline, with trailing
/// empty fields dropped. Empty lines in the middle are kept.
pub(crate) fn perl_lines(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    drop_trailing_empty(&mut lines);
    lines
}

/// Perl `split $pattern, $text` for a pattern without capture groups: the
/// fields between matches, with trailing empty fields dropped.
pub(crate) fn perl_split<'a>(pattern: &Regex, text: &'a str) -> Vec<&'a str> {
    let mut fields: Vec<&str> = pattern.split(text).collect();
    drop_trailing_empty(&mut fields);
    fields
}

fn drop_trailing_empty(fields: &mut Vec<&str>) {
    while fields.last().is_some_and(|field| field.is_empty()) {
        fields.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perl_lines_drops_only_trailing_empty_fields() {
        assert_eq!(perl_lines("a\n\nb\n\n\n"), vec!["a", "", "b"]);
        assert_eq!(perl_lines("\na"), vec!["", "a"]);
        assert!(perl_lines("").is_empty());
        assert!(perl_lines("\n\n").is_empty());
    }

    #[test]
    fn perl_split_drops_only_trailing_empty_fields() {
        let comma = Regex::new(r"\s*,\s*").unwrap();
        assert_eq!(perl_split(&comma, "a , ,b,,"), vec!["a", "", "b"]);
        assert_eq!(perl_split(&comma, ",a"), vec!["", "a"]);
        assert!(perl_split(&comma, "").is_empty());
    }
}
