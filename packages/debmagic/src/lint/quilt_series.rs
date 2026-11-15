//! Quilt series files under `debian/patches/`.
//!
//! Lintian's `debian/patches/quilt` Check reads a series file two ways.
//! `debian/patches/series` is read for the patches it applies: comments are
//! stripped, blank lines skipped, and each remaining line is a patch name with
//! optional options. Every series file (`series` and `<vendor>.series`) is
//! also read for the names it mentions: the first word of each line, after an
//! optional run of `#`, so a commented-out patch is still mentioned. Both
//! readings are kept on one object.

use std::sync::LazyLock;

use regex::Regex;

/// Perl's `\s` on bytes (lintian does not decode the series file).
const SPACE: &str = r"[\t\n\x0B\x0C\r ]";
/// Perl's `\S` on bytes: anything else, including non-ASCII.
const NON_SPACE: &str = r"[^\t\n\x0B\x0C\r ]";

/// `s/(?:^|\s+)#.*$//` — a comment at the start of the line or after
/// whitespace. `#` inside a word is not a comment.
static COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"(?:^|{SPACE}+)#.*")).expect("regex"));
/// `^(\S+)\s+(\S.*)$` on the trimmed line.
static PATCH_WITH_OPTIONS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"^({NON_SPACE}+){SPACE}+({NON_SPACE}.*)$")).expect("regex")
});
/// `^\s*(?:#+\s*)?(\S+)`, lintian's known-name rule for every series file.
static MENTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"^{SPACE}*(?:#+{SPACE}*)?({NON_SPACE}+)")).expect("regex")
});

/// A parsed quilt series file (`debian/patches/series` or
/// `debian/patches/<vendor>.series`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct QuiltSeries {
    patches: Vec<QuiltPatch>,
    mentions: Vec<String>,
}

/// One patch line of a series file, in series order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuiltPatch {
    name: String,
    options: Option<String>,
    line: u32,
}

impl QuiltSeries {
    /// Parse series text. Lines are split on `\n` as Perl's `<$fd>` does;
    /// a series file that is not UTF-8 is read lossily by the caller.
    pub fn parse(text: &str) -> Self {
        let mut series = Self::default();
        for (index, raw) in text.split_inclusive('\n').enumerate() {
            let line = u32::try_from(index + 1).unwrap_or(u32::MAX);
            let content = raw.strip_suffix('\n').unwrap_or(raw);
            if let Some(mention) = MENTION.captures(content) {
                series.mentions.push(mention[1].to_string());
            }
            let stripped = COMMENT.replace(content, "");
            let trimmed = trim_perl_space(&stripped);
            if trimmed.is_empty() {
                continue;
            }
            let (name, options) = match PATCH_WITH_OPTIONS.captures(trimmed) {
                Some(captures) => (captures[1].to_string(), Some(captures[2].to_string())),
                None => (trimmed.to_string(), None),
            };
            series.patches.push(QuiltPatch {
                name,
                options,
                line,
            });
        }
        series
    }

    /// Patches in series order, comments and blank lines left out.
    #[allow(dead_code)] // read by the other `debian/patches/quilt` Tags
    pub fn patches(&self) -> &[QuiltPatch] {
        &self.patches
    }

    /// The first word of each line, after an optional run of `#`, in file
    /// order. A commented-out patch is mentioned; so is the first word of a
    /// prose comment.
    pub fn mentions(&self) -> impl Iterator<Item = &str> {
        self.mentions.iter().map(String::as_str)
    }
}

#[allow(dead_code)] // read by the other `debian/patches/quilt` Tags
impl QuiltPatch {
    /// Patch path relative to `debian/patches/`, as written.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Everything after the name (`-p0`, `-p1 -R`), trimmed. `None` when the
    /// line is the name alone.
    pub fn options(&self) -> Option<&str> {
        self.options.as_deref()
    }

    /// 1-based line in the series file.
    pub fn line(&self) -> u32 {
        self.line
    }
}

/// Perl's `s/^\s+|\s+$//g` with ASCII `\s`.
fn trim_perl_space(text: &str) -> &str {
    text.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0B' | '\x0C'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patch(name: &str, options: Option<&str>, line: u32) -> QuiltPatch {
        QuiltPatch {
            name: name.to_string(),
            options: options.map(str::to_string),
            line,
        }
    }

    #[test]
    fn patches_skip_comments_and_blank_lines() {
        let series = QuiltSeries::parse(
            "# leading comment\n\
             \n\
             first.patch\n\
             \tsecond.patch   -p0  \n\
             third.patch # trailing comment\n\
             fourth#not-a-comment.patch\n\
             fifth.patch -p1 -R\n\
             last.patch",
        );
        assert_eq!(
            series.patches(),
            [
                patch("first.patch", None, 3),
                patch("second.patch", Some("-p0"), 4),
                patch("third.patch", None, 5),
                patch("fourth#not-a-comment.patch", None, 6),
                patch("fifth.patch", Some("-p1 -R"), 7),
                patch("last.patch", None, 8),
            ]
        );
    }

    #[test]
    fn mentions_include_commented_out_names() {
        let series = QuiltSeries::parse(
            "# tries to create an existing file\n\
             # some-file\n\
             some-other-file -p0\n\
             \n\
             ##   double-hash\n\
             #\n\
             \x20 indented # comment\n\
             #no-space\n",
        );
        assert_eq!(
            series.mentions().collect::<Vec<_>>(),
            [
                "tries",
                "some-file",
                "some-other-file",
                "double-hash",
                // A bare `#` backtracks out of the optional `#+` and is its
                // own first word, as in Perl.
                "#",
                "indented",
                "no-space"
            ]
        );
    }

    #[test]
    fn empty_text_has_nothing() {
        let series = QuiltSeries::parse("");
        assert!(series.patches().is_empty());
        assert_eq!(series.mentions().count(), 0);
    }

    #[test]
    fn crlf_is_whitespace() {
        let series = QuiltSeries::parse("a.patch\r\n# b.patch\r\n");
        assert_eq!(series.patches(), [patch("a.patch", None, 1)]);
        assert_eq!(
            series.mentions().collect::<Vec<_>>(),
            ["a.patch", "b.patch"]
        );
    }
}
