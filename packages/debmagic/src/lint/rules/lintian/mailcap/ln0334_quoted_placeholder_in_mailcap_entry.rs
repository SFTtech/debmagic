use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

pub struct QuotedPlaceholderInMailcapEntry;

declare_rule! {
    /// A mailcap entry quotes a `%s` placeholder.
    ///
    /// Files under `usr/lib/mime/packages/` are read. A trailing `\` joins the
    /// next physical line, and the hit points at the first line of that entry.
    /// Blank lines and comments are skipped only outside a continuation.
    /// Single- and double-quoted strings are extracted with backslash escapes.
    /// Repeated identical quotes are reported once, in order. The extra is
    /// those quotes separated by spaces. Location is the mailcap file.
    QuotedPlaceholderInMailcapEntry,
    code = "LN0334",
    tag = "quoted-placeholder-in-mailcap-entry",
    default_selected = true,
    experimental = false,
}

static MAILCAP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u)^usr/lib/mime/packages/").expect("valid regex"));

impl BinaryPackageRule for QuotedPlaceholderInMailcapEntry {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let hits = {
            let Some(files) = ctx.data_files() else {
                return;
            };
            mailcap_hits(files)
        };
        for (path, line, extra) in hits {
            ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

fn mailcap_hits(files: &FileTree) -> Vec<(PathBuf, u32, String)> {
    let mut hits = Vec::new();
    for entry in files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .path(&MAILCAP)
    {
        let Ok(bytes) = files.read(entry) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        for (line, assembled) in entries(&text) {
            let quoted = quoted_percent_s(assembled.as_bytes());
            if quoted.is_empty() {
                continue;
            }
            hits.push((entry.path().to_path_buf(), line, quoted.join(" ")));
        }
    }
    hits
}

/// Logical mailcap entries and the physical line where each one starts.
fn entries(text: &str) -> Vec<(u32, String)> {
    let mut found = Vec::new();
    let mut parts: Vec<String> = Vec::new();
    let mut start = 1u32;
    for (index, raw) in physical_lines(text).into_iter().enumerate() {
        let position = u32::try_from(index + 1).unwrap_or(u32::MAX);
        if parts.is_empty() && (is_blank(raw) || is_comment(raw)) {
            continue;
        }
        let (continued, body) = split_continuation(raw);
        if parts.is_empty() {
            start = position;
        }
        parts.push(body);
        if continued {
            continue;
        }
        found.push((start, parts.join("\n")));
        parts.clear();
    }
    found
}

fn quoted_percent_s(bytes: &[u8]) -> Vec<String> {
    let mut found = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if let Some((quoted, consumed)) = extract_quoted(&bytes[index..]) {
            if quoted.contains("%s") && !found.contains(&quoted) {
                found.push(quoted);
            }
            index += consumed.max(1);
        } else {
            index += 1;
        }
    }
    found
}

/// Text::Balanced `extract_delimited` for `'` and `"`, prefix `[^'"]*`.
fn extract_quoted(bytes: &[u8]) -> Option<(String, usize)> {
    let mut index = 0;
    while index < bytes.len() && bytes[index] != b'\'' && bytes[index] != b'"' {
        index += 1;
    }
    if index >= bytes.len() {
        return None;
    }
    let delimiter = bytes[index];
    let start = index;
    index += 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            if index + 1 >= bytes.len() {
                return None;
            }
            index += 2;
            continue;
        }
        if bytes[index] == delimiter {
            index += 1;
            let quoted = String::from_utf8_lossy(&bytes[start..index]).into_owned();
            return Some((quoted, index));
        }
        index += 1;
    }
    None
}

fn split_continuation(line: &str) -> (bool, String) {
    let line = line.trim_end_matches('\r');
    if let Some(stripped) = line.strip_suffix('\\') {
        (true, stripped.to_string())
    } else {
        (false, line.to_string())
    }
}

fn physical_lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<&str> = text.split('\n').collect();
    if text.ends_with('\n') {
        lines.pop();
    }
    lines
}

fn is_blank(line: &str) -> bool {
    line.bytes()
        .all(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0c))
}

fn is_comment(line: &str) -> bool {
    line.bytes()
        .find(|byte| !matches!(byte, b' ' | b'\t' | b'\r' | 0x0c))
        == Some(b'#')
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn mailcap(name: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(mailcap_path(name), TestFile::new(body))])
    }

    fn mailcap_path(name: &str) -> String {
        format!("usr/lib/mime/packages/{name}")
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            mailcap(
                "lynx-common",
                "text/html; /usr/bin/lynx -force_html %s; nametemplate=%s.html\n",
            ),
            mailcap("plain", "text/plain; cat '%t'; needsterminal\n"),
            mailcap("note", "# less '%s'\n\n"),
            BinaryPackageCase::from(vec![(
                "usr/share/mime/packages/less",
                TestFile::new("text/plain; less '%s'\n"),
            )]),
            BinaryPackageCase::from(vec![(mailcap_path("less"), TestFile::symlink("missing"))]),
        ];
        let fail = vec![
            mailcap("less", "text/plain; less '%s'; needsterminal; priority=8\n"),
            mailcap(
                "imagemagick-6.q16",
                "image/png; display-im6.q16 'png:%s'; test=test -n \"$DISPLAY\"; priority=2\n",
            ),
            mailcap("double", "text/plain; less \"%s\"; needsterminal\n"),
            mailcap("both", "text/plain; less '%s' \"%s\"; needsterminal\n"),
            mailcap("dup", "text/plain; less '%s'; other '%s'\n"),
            mailcap("escape", "text/plain; less 'it\\'s %s';\n"),
            mailcap("cont", "text/plain; less \\\n  '%s'; needsterminal\n"),
            mailcap("later", "# comment\n\ntext/plain; less '%s'\n"),
        ];
        BinaryPackageTester::new(QuotedPlaceholderInMailcapEntry, pass, fail).test_and_snapshot();
    }
}
