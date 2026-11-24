//! Text helpers that follow Perl semantics, shared by the Rules that port
//! lintian's line and field walks.

use regex::Regex;

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
