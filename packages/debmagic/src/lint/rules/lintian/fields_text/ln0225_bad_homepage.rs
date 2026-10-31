use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

pub struct BadHomepage;

declare_rule! {
    /// `Homepage` is not an absolute URI with scheme `http`, `https`, `ftp`,
    /// or `gopher`.
    ///
    /// Reads the source paragraph of `debian/control`, and Installation control
    /// on a Binary package. Angle brackets (optional `URL:` / `URI:` inside)
    /// are stripped before the scheme check. Extra is the field value with
    /// surrounding space trimmed. Installable paragraphs and `.changes` are
    /// not read. Udebs are checked too.
    BadHomepage,
    code = "LN0225",
    tag = "bad-homepage",
    default_selected = true,
    experimental = false,
}

static CLUTTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^<(?:UR[LI]:)?.*>$").expect("valid regex"));

impl SourceRule for BadHomepage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        if let Some(message) = bad_homepage(control.source()) {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

impl BinaryPackageRule for BadHomepage {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if let Some(message) = bad_homepage(ctx.installation_control().fields()) {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

fn bad_homepage(fields: &ControlParagraph) -> Option<String> {
    if !fields.declares("Homepage") {
        return None;
    }
    let original = fields
        .untrimmed_value("Homepage")
        .unwrap_or("")
        .trim()
        .to_string();
    let mut unfolded = original.replace('\n', "");
    unfolded = unfolded.trim_start().to_string();
    if CLUTTER.is_match(&unfolded) {
        let mut chars = unfolded.chars();
        chars.next();
        chars.next_back();
        unfolded = chars.as_str().to_string();
    }
    let bad = match uri_scheme(&unfolded) {
        Some(scheme) => !matches!(scheme.as_str(), "ftp" | "http" | "https" | "gopher"),
        None => true,
    };
    bad.then_some(original)
}

/// Scheme of an absolute URI, lowercased, as `URI->scheme` reads it.
fn uri_scheme(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    if bytes.first().is_none_or(|byte| !byte.is_ascii_alphabetic()) {
        return None;
    }
    let mut end = 1usize;
    while end < bytes.len()
        && (bytes[end].is_ascii_alphanumeric() || matches!(bytes[end], b'+' | b'-' | b'.'))
    {
        end += 1;
    }
    if bytes.get(end) == Some(&b':') {
        Some(value[..end].to_ascii_lowercase())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(homepage_line: &str) -> Vec<(&'static str, TestFile)> {
        let homepage = if homepage_line.is_empty() {
            String::new()
        } else {
            format!("{homepage_line}\n")
        };
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: Example <ex@example.com>\n\
                 Standards-Version: 4.7.2\n{homepage}\
                 \n\
                 Package: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    fn binary(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Homepage", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            control(""),
            control("Homepage: https://example.com/"),
            control("Homepage: http://lintian.debian.org/"),
            control("Homepage: HTTP://example.com/"),
            control("Homepage: ftp://example.com/pub"),
            control("Homepage: gopher://example.com/"),
            control("Homepage: <http://lintian.debian.org/>"),
            // A bad URL on an installable paragraph is the binary package's.
            vec![(
                "debian/control",
                TestFile::new(
                    "Source: example\nMaintainer: Example <ex@example.com>\n\
                     Standards-Version: 4.7.2\nHomepage: https://example.com/\n\
                     \n\
                     Package: example\nArchitecture: all\nHomepage: lintian.debian.org\n\
                     Description: example\n extra\n",
                ),
            )],
            vec![("debian/control", TestFile::new("not a deb822 file\n: \n"))],
        ];
        let fail = vec![
            control("Homepage: lintian.debian.org"),
            control("Homepage: ttp://lintian.debian.org/"),
            control("Homepage: file:///etc/passwd"),
            control("Homepage: git://example.com/foo"),
            control("Homepage:"),
            control("Homepage: <not-a-url>"),
            control("Homepage: <URL:https://example.com/>"),
        ];
        SourceTreeTester::new(BadHomepage, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            binary("https://example.com/"),
            binary("http://lintian.debian.org/"),
            binary("<http://lintian.debian.org/>"),
            binary("gopher://example.com/"),
        ];
        let fail = vec![
            binary("lintian.debian.org"),
            binary("ttp://lintian.debian.org/"),
            binary("file:///tmp/x"),
            binary("<URL:https://example.com/>"),
            binary("").udeb(),
        ];
        BinaryPackageTester::new(BadHomepage, pass, fail).test_and_snapshot_with_suffix("binary");
    }
}
