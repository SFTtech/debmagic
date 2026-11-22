use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::{BinaryPackageRule, Severity};
use crate::lint::rules::lintian::init_scripts::systemd_unit;

pub struct InvalidSystemdDocumentation;

declare_rule! {
    /// A `Documentation=` value in a service unit's `[Unit]` section is not a URI.
    ///
    /// Units are the regular files and hardlinks named `*.service` under
    /// `systemd/system/`, as for the missing-documentation Tag. Each value is
    /// split into shell words, then each word is split on whitespace, as
    /// lintian does; every word that is not a URI is reported. A URI here is
    /// an RFC 3986 absolute URI: a scheme, then characters allowed by the
    /// grammar, with `%` only as a two-digit hex escape (so `%i` is not a
    /// URI). Reported as an info hint with the word as Extra; the unit path
    /// is the Location. Values in other sections do not count.
    InvalidSystemdDocumentation,
    code = "LN1584",
    tag = "invalid-systemd-documentation",
    default_selected = false,
    experimental = false,
}

static SERVICE_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/systemd/system/.*\.service$").expect("valid regex"));

static WHITESPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").expect("valid regex"));

impl BinaryPackageRule for InvalidSystemdDocumentation {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for (path, word) in invalid_documentation(files) {
            ctx.diagnostic(
                Diagnostic::warn(word)
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

/// `(unit path, invalid word)` pairs, in tree order.
fn invalid_documentation(files: &FileTree) -> Vec<(PathBuf, String)> {
    let mut invalid = Vec::new();
    for entry in files.entries() {
        if !SERVICE_PATH.is_match(&entry.path().to_string_lossy()) {
            continue;
        }
        if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
            continue;
        }
        let Some(text) = service_text(files, entry) else {
            continue;
        };
        let lines = systemd_unit::service_lines(&text);
        for value in systemd_unit::service_values(&lines, "Unit", "Documentation") {
            for word in perl_split_whitespace(&value) {
                if !is_uri(word) {
                    invalid.push((entry.path().to_path_buf(), word.to_string()));
                }
            }
        }
    }
    invalid
}

/// Perl's `split(m{\s+}, $text)`: a leading empty field is kept, trailing
/// empty fields are dropped, and an empty text has no fields.
fn perl_split_whitespace(text: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = WHITESPACE.split(text).collect();
    while parts.last().is_some_and(|part| part.is_empty()) {
        parts.pop();
    }
    parts
}

fn service_text(files: &FileTree, entry: &crate::lint::file_tree::Entry) -> Option<String> {
    let bytes = files.read(entry).ok()?;
    String::from_utf8(bytes).ok()
}

/// An RFC 3986 absolute URI: `scheme ":" rest`, where the scheme starts with
/// a letter, and `rest` uses unreserved, sub-delims, `:@/?#[]`, and `%XX`.
fn is_uri(word: &str) -> bool {
    let Some((scheme, rest)) = word.split_once(':') else {
        return false;
    };
    let mut scheme_chars = scheme.chars();
    let scheme_ok = scheme_chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && scheme_chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'));
    if !scheme_ok {
        return false;
    }
    let bytes = rest.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'%' {
            let escaped = bytes.get(index + 1..index + 3);
            if !escaped.is_some_and(|hex| hex.iter().all(u8::is_ascii_hexdigit)) {
                return false;
            }
            index += 3;
            continue;
        }
        let allowed = byte.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@/?#[]".contains(&byte);
        if !allowed {
            return false;
        }
        index += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn unit(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/lib/systemd/system/example.service",
            TestFile::new(body),
        )])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            unit("[Unit]\nDocumentation=https://example.com/ man:example(8)\n"),
            unit("[Unit]\nDocumentation=file:///usr/share/doc/x%20y\n"),
            // A missing value is not this Tag; an empty assignment resets it.
            unit("[Unit]\nDocumentation=\n"),
            // Values in another section are not checked.
            unit("[Service]\nDocumentation=https://example.com/%i/\n"),
            // A masked unit is not a regular file.
            BinaryPackageCase::from(vec![(
                "usr/lib/systemd/system/example.service",
                TestFile::symlink("/dev/null"),
            )]),
        ];
        let fail = vec![
            unit("[Unit]\nDocumentation=https://example.com/%i/\n"),
            unit("[Unit]\nDocumentation=example.com/docs\n"),
            unit("[Unit]\nDocumentation=\"man:a b\" https://example.com/%zz\n"),
            BinaryPackageCase::from(vec![(
                "usr/lib/systemd/system/example.service",
                TestFile::new("[Unit]\nDocumentation=https://example.com/%i/\n"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(InvalidSystemdDocumentation, pass, fail).test_and_snapshot();
    }
}
