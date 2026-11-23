//! The diffstat of a Source package's Debian diff: the files that
//! `<name>_<version>.diff.gz` changes, as lintian's `processable->diffstat`
//! lists them. Lintian runs `diffstat -p1` over the gunzipped diff and keys its
//! hash by the file names (the stats text is not needed by any Rule).
//!
//! This module reads the unified diff itself: a `---` header followed by a
//! `+++` header starts a file section, hunk bodies are skipped by their counts
//! so a removed line such as `-- x` is not mistaken for a header, and the
//! `-p1` strip drops the first path component. A creation (`--- /dev/null`)
//! names the file by its `+++` path; a deletion (`+++ /dev/null`) by its `---`
//! path.

use std::collections::BTreeSet;
use std::io::Read;

use flate2::read::MultiGzDecoder;

/// The files a Debian diff changes, relative to the tree root (`-p1`), sorted
/// and without duplicates.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Diffstat {
    files: Vec<String>,
}

impl Diffstat {
    /// Files of the unified diff `text`. A section whose names are both
    /// `/dev/null`, or whose name is empty after the `-p1` strip, is left out.
    /// Renames are listed under their new name only (diffstat's `{a => b}`
    /// form is not reproduced).
    pub fn parse(text: &str) -> Self {
        let mut files = BTreeSet::new();
        let mut lines = text.lines().peekable();
        while let Some(line) = lines.next() {
            if let Some(counts) = line.strip_prefix("@@ ").and_then(hunk_counts) {
                skip_hunk_body(&mut lines, counts);
                continue;
            }
            let Some(old) = line.strip_prefix("--- ") else {
                continue;
            };
            let Some(new) = lines.peek().and_then(|next| next.strip_prefix("+++ ")) else {
                continue;
            };
            lines.next();
            if let Some(name) = header_name(new).or_else(|| header_name(old)) {
                files.insert(name);
            }
        }
        Self {
            files: files.into_iter().collect(),
        }
    }

    /// Parse the gzip-compressed diff `bytes` as lintian's `gunzip --stdout`
    /// does: a corrupt stream keeps the bytes read before the error.
    pub fn from_gzip(bytes: &[u8]) -> Self {
        let mut plain = Vec::new();
        let _ = MultiGzDecoder::new(bytes).read_to_end(&mut plain);
        Self::parse(&String::from_utf8_lossy(&plain))
    }

    /// The files the diff changes, sorted.
    pub fn files(&self) -> &[String] {
        &self.files
    }

    /// Whether the diff changes no file (no `.diff.gz`, or none that parses).
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// The old and new hunk line counts of a `@@ -a[,b] +c[,d] @@` header (the text
/// after `@@ `). A missing count is 1.
fn hunk_counts(header: &str) -> Option<(usize, usize)> {
    let mut parts = header.split_whitespace();
    let old = count_of(parts.next()?.strip_prefix('-')?)?;
    let new = count_of(parts.next()?.strip_prefix('+')?)?;
    Some((old, new))
}

fn count_of(range: &str) -> Option<usize> {
    match range.split_once(',') {
        Some((_, count)) => count.parse().ok(),
        None => Some(1),
    }
}

/// Consume the body of a hunk: context lines count for both sides, `-` lines
/// for the old side, `+` lines for the new side, and `\` lines for neither. The
/// body ends when both counts are used up, or early at a line that is not hunk
/// text.
fn skip_hunk_body<'a>(
    lines: &mut std::iter::Peekable<std::str::Lines<'a>>,
    (mut old, mut new): (usize, usize),
) {
    while old > 0 || new > 0 {
        let Some(line) = lines.peek() else {
            return;
        };
        match line.as_bytes().first() {
            Some(b' ') | None => {
                old = old.saturating_sub(1);
                new = new.saturating_sub(1);
            }
            Some(b'-') => old = old.saturating_sub(1),
            Some(b'+') => new = new.saturating_sub(1),
            Some(b'\\') => {}
            Some(_) => return,
        }
        lines.next();
    }
}

/// The `-p1` name of a `---` or `+++` header field (the text after the marker),
/// or `None` for `/dev/null` and an empty name. A tab starts the timestamp.
fn header_name(field: &str) -> Option<String> {
    let path = field.split('\t').next()?.trim_end();
    if path.is_empty() || path == "/dev/null" {
        return None;
    }
    let stripped = match path.split_once('/') {
        Some((_, rest)) => rest,
        None => path,
    };
    (!stripped.is_empty()).then(|| stripped.to_string())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::*;

    const TWO_FILES: &str = "\
--- pkg-1.0.orig/README
+++ pkg-1.0/README
@@ -1,2 +1,2 @@
-old
+new
 same
--- pkg-1.0.orig/debian/changelog
+++ pkg-1.0/debian/changelog
@@ -1 +1 @@
-a
+b
";

    #[test]
    fn lists_each_file_stripped_of_its_first_component_sorted() {
        let diffstat = Diffstat::parse(TWO_FILES);
        assert_eq!(diffstat.files(), ["README", "debian/changelog"]);
        assert!(!diffstat.is_empty());
    }

    #[test]
    fn creation_uses_new_name_and_deletion_uses_old_name() {
        let text = "\
--- /dev/null
+++ pkg-1.0/new.txt
@@ -0,0 +1 @@
+x
--- pkg-1.0/gone.txt\t2020-01-01 00:00:00.000000000 +0000
+++ /dev/null
@@ -1 +0,0 @@
-x
--- /dev/null
+++ /dev/null
";
        assert_eq!(
            Diffstat::parse(text).files(),
            ["gone.txt", "new.txt"],
            "both-null sections are left out"
        );
    }

    #[test]
    fn timestamp_after_tab_is_not_part_of_the_name() {
        let text = "--- a/x.txt\t2001-01-01\n+++ b/x.txt\t2002-02-02\n@@ -1 +1 @@\n-a\n+b\n";
        assert_eq!(Diffstat::parse(text).files(), ["x.txt"]);
    }

    #[test]
    fn hunk_bodies_are_skipped_by_their_counts() {
        // A removed line `-- file` looks like a `---` header, and an added
        // `++ x` looks like `+++`, but both are hunk text.
        let text = "\
--- a/real.txt
+++ b/real.txt
@@ -1,3 +1,3 @@
 keep
--- a/fake.txt
-- a/fake.txt
+++ b/fake.txt
++ b/fake.txt
@@ -5 +5 @@
-x
+y
";
        assert_eq!(Diffstat::parse(text).files(), ["real.txt"]);
    }

    #[test]
    fn no_newline_marker_and_duplicates() {
        let text = "--- a/f\n+++ b/f\n@@ -1 +1 @@\n-a\n\\ No newline at end of file\n+b\n\
                    --- a/f\n+++ b/f\n@@ -1 +1 @@\n-c\n+d\n";
        assert_eq!(Diffstat::parse(text).files(), ["f"]);
    }

    #[test]
    fn name_without_a_directory_is_kept_and_empty_input_lists_nothing() {
        assert_eq!(
            Diffstat::parse("--- a\n+++ b\n@@ -1 +1 @@\n-x\n+y\n").files(),
            ["b"]
        );
        assert!(Diffstat::parse("").is_empty());
        assert!(Diffstat::parse("not a diff\n").files().is_empty());
    }

    #[test]
    fn gzip_reads_all_members_and_keeps_bytes_before_corruption() {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(TWO_FILES.as_bytes()).unwrap();
        let gz = encoder.finish().unwrap();
        assert_eq!(
            Diffstat::from_gzip(&gz).files(),
            ["README", "debian/changelog"]
        );

        let mut truncated = gz.clone();
        truncated.truncate(truncated.len() - 8);
        let kept = Diffstat::from_gzip(&truncated);
        assert_eq!(kept.files().first().map(String::as_str), Some("README"));

        assert!(Diffstat::from_gzip(b"plain text, not gzip").is_empty());
    }
}
