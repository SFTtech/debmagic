use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{Entry, FileTree};
use crate::lint::interpreter::Hashbang;
use crate::lint::rule::{Severity, SourceRule};

pub struct UnicodeTrojan;

declare_rule! {
    /// A file name or a script line contains a bidirectional or homoglyph
    /// control character from the Trojan Source attacks.
    ///
    /// Walks the packaging File tree (Source tree, or Source package Patched
    /// files) except the quilt `.pc/` directory. Each entry whose path is valid
    /// UTF-8 is checked for the twelve codepoints of lintian's list (ARABIC
    /// LETTER MARK, the LEFT-TO-RIGHT and RIGHT-TO-LEFT marks, embeddings,
    /// overrides, and isolates). Each hit is reported as `File name`, the
    /// codepoint as `U+` and hex without padding, as lintian's `%vX`, and the
    /// name in double quotes. Regular files and hardlinks whose content starts
    /// with a `#!` hashbang are then read line by line, and each line that is
    /// valid UTF-8 is checked the same way, reported as `Contents` with the
    /// line as Location. Pedantic, experimental and not selected by default,
    /// as in lintian. Extra is that text.
    UnicodeTrojan,
    code = "LN1763",
    tag = "unicode-trojan",
    default_selected = false,
    experimental = true,
}

/// lintian's `%NAMES_BY_CHARACTER`, in the order they are checked here (lintian
/// uses hash order).
const CHARACTERS: [(char, &str); 12] = [
    ('\u{061C}', "ARABIC LETTER MARK"),
    ('\u{200E}', "LEFT-TO-RIGHT MARK"),
    ('\u{200F}', "RIGHT-TO-LEFT MARK"),
    ('\u{202A}', "LEFT-TO-RIGHT EMBEDDING"),
    ('\u{202B}', "RIGHT-TO-LEFT EMBEDDING"),
    ('\u{202C}', "POP DIRECTIONAL FORMATTING"),
    ('\u{202D}', "LEFT-TO-RIGHT OVERRIDE"),
    ('\u{202E}', "RIGHT-TO-LEFT OVERRIDE"),
    ('\u{2066}', "LEFT-TO-RIGHT ISOLATE"),
    ('\u{2067}', "RIGHT-TO-LEFT ISOLATE"),
    ('\u{2068}', "FIRST STRONG ISOLATE"),
    ('\u{2069}', "POP DIRECTIONAL ISOLATE"),
];

impl SourceRule for UnicodeTrojan {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let mut hits = Vec::new();
        for entry in files.entries() {
            if entry.path().starts_with(".pc") {
                continue;
            }
            check_name(entry, &mut hits);
            if entry.is_file() {
                check_script(files, entry, &mut hits);
            }
        }
        for hit in hits {
            ctx.diagnostic(
                Diagnostic::warn(hit.extra)
                    .with_location(Location {
                        path: hit.path,
                        line: hit.line,
                        column: None,
                    })
                    .with_severity(Severity::Pedantic),
            );
        }
    }
}

struct Hit {
    extra: String,
    path: std::path::PathBuf,
    line: Option<u32>,
}

fn check_name(entry: &Entry, hits: &mut Vec<Hit>) {
    let Some(name) = entry.path().to_str() else {
        return;
    };
    for (character, label) in CHARACTERS {
        if name.contains(character) {
            hits.push(Hit {
                extra: describe("File name", character, label),
                path: entry.path().to_path_buf(),
                line: None,
            });
        }
    }
}

fn check_script(files: &FileTree, entry: &Entry, hits: &mut Vec<Hit>) {
    let Ok(bytes) = files.read(entry) else {
        return;
    };
    if Hashbang::read(&bytes).is_none() {
        return;
    }
    for (index, line) in bytes.split(|&byte| byte == b'\n').enumerate() {
        let Ok(text) = std::str::from_utf8(line) else {
            continue;
        };
        for (character, label) in CHARACTERS {
            if text.contains(character) {
                hits.push(Hit {
                    extra: describe("Contents", character, label),
                    path: entry.path().to_path_buf(),
                    line: Some(u32::try_from(index + 1).unwrap_or(u32::MAX)),
                });
            }
        }
    }
}

fn describe(place: &str, character: char, label: &str) -> String {
    format!("{place} U+{:X} \"{label}\"", u32::from(character))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageTester, TestFile};

    use super::*;

    fn tree(extra: &[(&str, TestFile)]) -> Vec<(String, TestFile)> {
        extra
            .iter()
            .map(|(path, file)| (path.to_string(), file.clone()))
            .collect()
    }

    #[test]
    fn source() {
        let pass = vec![
            tree(&[]),
            tree(&[("scripts/plain", TestFile::new("plain\n"))]),
            // Not a script: the contents are not read.
            tree(&[("data/text", TestFile::new("potential\u{202B}trojan\n"))]),
            // Invalid UTF-8 lines are skipped.
            tree(&[(
                "scripts/bad",
                TestFile::from_bytes(b"#!/bin/sh\n\xff\xfe\n".to_vec()),
            )]),
            // Quilt's `.pc` directory is not visited.
            tree(&[(
                ".pc/patch/scripts/example\u{202E}fig.exe",
                TestFile::new("text\n"),
            )]),
        ];
        let fail = vec![
            tree(&[("scripts/example\u{202E}fig.exe", TestFile::new("text\n"))]),
            tree(&[(
                "scripts/careful",
                TestFile::new("#!/bin/sh\n\npotential\u{202B}trojan\n"),
            )]),
            tree(&[(
                "scripts/both\u{2066}",
                TestFile::new("#!/bin/sh\n\u{2069}\n"),
            )]),
        ];
        SourcePackageTester::new(UnicodeTrojan, pass, fail).test_and_snapshot();
    }
}
