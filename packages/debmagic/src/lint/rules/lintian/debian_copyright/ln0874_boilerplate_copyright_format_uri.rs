use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_copyright::DebianCopyright;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dep5_scan::copyright_texts;

pub struct BoilerplateCopyrightFormatUri;

declare_rule! {
    /// A machine-readable copyright names a boilerplate Format URI.
    ///
    /// Reads `debian/copyright` and `debian/<installable>.copyright` (symlinks
    /// and files that are not UTF-8 are skipped). The Format URI of the first
    /// paragraph, without any `#fragment`, is reported when it contains
    /// `VERSIONED_FORMAT_URL` or `rev=REVISION`. Extra is the URI; the Location
    /// is the copyright file.
    BoilerplateCopyrightFormatUri,
    code = "LN0874",
    tag = "boilerplate-copyright-format-uri",
    default_selected = true,
    experimental = false,
}

impl SourceRule for BoilerplateCopyrightFormatUri {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for (path, text) in copyright_texts(ctx) {
            // Only a machine-readable marker with a boilerplate URI reaches
            // lintian's boilerplate branch; every other format is left alone.
            if DebianCopyright::parse(&text) != DebianCopyright::UnknownFormat {
                continue;
            }
            let Some(uri) = format_uri(&text) else {
                continue;
            };
            if !BOILERPLATE.is_match(&uri) {
                continue;
            }
            ctx.diagnostic(Diagnostic::warn(uri).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

static COMMENT_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^#.*").expect("valid regex"));
static TRAILING_BLANKS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)[ \t]+$").expect("valid regex"));
static FOLDED_NEWLINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\n?[ \t]+").expect("valid regex"));
static FORMAT_FIELD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?mi)^Format(?:-Specification)?:\s*(\S+)\s*$").expect("valid regex")
});
static BOILERPLATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:rev=REVISION|VERSIONED_FORMAT_URL)\b").expect("valid regex")
});

/// The Format URI of the first paragraph, as `Dep5.pm` extracts it, with any
/// `#fragment` stripped.
fn format_uri(text: &str) -> Option<String> {
    let text = COMMENT_LINE.replace_all(text, "");
    let text = TRAILING_BLANKS.replace_all(&text, "");
    let mut first = text.trim_start_matches('\n').to_string();
    if let Some(end) = first.find("\n\n") {
        first.truncate(end + 1);
    }
    let folded = FOLDED_NEWLINE.replace_all(&first, " ");
    let uri = FORMAT_FIELD.captures(&folded)?.get(1)?.as_str();
    Some(match uri.split_once('#') {
        Some((base, _)) if !base.is_empty() => base.to_string(),
        _ => uri.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::SourceTreeTester;

    use super::*;

    const DEP5: &str = "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n";

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/copyright", DEP5)],
            vec![(
                "debian/copyright",
                "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/#fragment\n\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n",
            )],
            // A boilerplate URI outside the first paragraph is not the Format.
            vec![(
                "debian/copyright",
                "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n\nFormat: <VERSIONED_FORMAT_URL>\n",
            )],
            // A valid URI in any copyright file is fine.
            vec![("debian/copyright", DEP5), ("debian/bar.copyright", DEP5)],
        ];
        let fail = vec![
            vec![(
                "debian/copyright",
                "Format: <VERSIONED_FORMAT_URL>\n\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n",
            )],
            vec![(
                "debian/copyright",
                "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/rev=REVISION\n\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n",
            )],
            vec![(
                "debian/copyright",
                "Format: https://example.org/copyright-format/VERSIONED_FORMAT_URL#frag\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n",
            )],
        ];
        SourceTreeTester::new(BoilerplateCopyrightFormatUri, pass, fail).test_and_snapshot();
    }
}
