//! Lintian override files, parsed but not applied.
//!
//! Mirrors `Lintian::Processable::Overrides::parse_overrides` and the
//! `override_file` lookups of `Lintian::Processable::{Installable,Source}::Overrides`
//! in lintian 2.139.0. Nothing here suppresses a Diagnostic.

use std::path::{Path, PathBuf};

use super::diagnostic::ProcessableType;
use super::file_tree::{EntryKind, FileTree};

/// One Lintian override file of a Subject, parsed.
///
/// Lines that do not parse are left out of [`Self::overrides`] and kept in
/// [`Self::errors`] with lintian's message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LintianOverrides {
    path: PathBuf,
    overrides: Vec<LintianOverride>,
    errors: Vec<OverrideError>,
}

/// One override line: `[[package] [[arch …]] [type]:] tag [context]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LintianOverride {
    tag: String,
    architectures: Vec<String>,
    context: String,
    line: u32,
    justification: String,
}

/// A line lintian could not read as an override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OverrideError {
    line: u32,
    message: String,
}

impl LintianOverrides {
    /// The installed override file of a Binary package:
    /// `usr/share/lintian/overrides/<package>`, else the same with `.gz`.
    /// `None` when neither exists. A file that is not UTF-8 (or a `.gz` that
    /// does not decompress to UTF-8) has no overrides, as in lintian.
    pub fn installed(
        files: &FileTree,
        package: &str,
        processable_type: ProcessableType,
    ) -> Option<Self> {
        let plain = format!("usr/share/lintian/overrides/{package}");
        let candidates = [PathBuf::from(&plain), PathBuf::from(format!("{plain}.gz"))];
        let (path, bytes) = first_present(files, &candidates)?;
        let bytes = if path.extension().is_some_and(|extension| extension == "gz") {
            debmagic_common::compression::gunzip(&bytes).unwrap_or_default()
        } else {
            bytes
        };
        let text = std::str::from_utf8(&bytes).unwrap_or_default();
        Some(Self::parse(path, text, package, processable_type))
    }

    /// The override file of a Source tree or Source package's Patched files:
    /// `debian/source/lintian-overrides`, else `debian/source.lintian-overrides`.
    /// `None` when neither exists. A file that is not UTF-8 has no overrides.
    pub fn source(files: &FileTree, source_name: &str) -> Option<Self> {
        let candidates = [
            PathBuf::from("debian/source/lintian-overrides"),
            PathBuf::from("debian/source.lintian-overrides"),
        ];
        let (path, bytes) = first_present(files, &candidates)?;
        let text = std::str::from_utf8(&bytes).unwrap_or_default();
        Some(Self::parse(
            path,
            text,
            source_name,
            ProcessableType::Source,
        ))
    }

    /// Parse override text. `package` and `processable_type` are the prefixes
    /// a line may name; a line naming another package keeps that name in its
    /// Tag, as lintian does (#699628).
    pub fn parse(
        path: PathBuf,
        text: &str,
        package: &str,
        processable_type: ProcessableType,
    ) -> Self {
        let mut overrides: Vec<LintianOverride> = Vec::new();
        let mut errors = Vec::new();
        let mut justification = String::new();
        // Tag of the previous override in the same block, for carrying over
        // its justification. A blank line ends the block.
        let mut previous: Option<usize> = None;

        for (index, line) in text.split('\n').enumerate() {
            let position = u32::try_from(index + 1).unwrap_or(u32::MAX);
            let trimmed = line.trim();
            if trimmed.is_empty() {
                justification.clear();
                previous = None;
                continue;
            }
            if let Some(comment) = trimmed.strip_prefix('#') {
                let comment = comment.trim_start();
                if !comment.is_empty() {
                    if !justification.is_empty() {
                        justification.push(' ');
                    }
                    justification.push_str(comment);
                }
                continue;
            }

            let collapsed = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
            let mut remaining = collapsed.as_str();
            let mut require_colon = false;
            let mut architectures = Vec::new();

            if let Some(rest) = strip_word(remaining, package) {
                remaining = rest.trim_start();
                require_colon = true;
            }
            if let Some((list, rest)) = strip_architectures(remaining) {
                architectures = list.split_whitespace().map(str::to_string).collect();
                remaining = rest.trim_start();
                require_colon = true;
            }
            if let Some(rest) = strip_word(remaining, processable_type.as_str()) {
                remaining = rest.trim_start();
                require_colon = true;
            }
            if require_colon {
                match remaining.trim_start().strip_prefix(':') {
                    Some(rest) => remaining = rest.trim_start(),
                    None => {
                        errors.push(OverrideError {
                            line: position,
                            message: "Expected a colon".to_string(),
                        });
                        continue;
                    }
                }
            }

            let (tag, context) = match remaining.split_once(' ') {
                Some((tag, context)) => (tag, context),
                None => (remaining, ""),
            };
            if tag.is_empty() {
                errors.push(OverrideError {
                    line: position,
                    message: format!("Cannot parse line: {line}"),
                });
                continue;
            }

            if justification.is_empty()
                && let Some(previous) = previous.map(|index| &overrides[index])
                && previous.tag == tag
            {
                justification = previous.justification.clone();
            }
            let justification = std::mem::take(&mut justification)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");

            overrides.push(LintianOverride {
                tag: tag.to_string(),
                architectures,
                context: context.to_string(),
                line: position,
                justification,
            });
            previous = Some(overrides.len() - 1);
        }

        Self {
            path,
            overrides,
            errors,
        }
    }

    /// Path of the override file in its File tree.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Overrides in file order.
    pub fn overrides(&self) -> &[LintianOverride] {
        &self.overrides
    }

    /// Lines that are not overrides, in file order.
    pub fn errors(&self) -> &[OverrideError] {
        &self.errors
    }
}

impl LintianOverride {
    /// The Tag name as written. It may be an old name, or no Tag at all.
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// Architectures and wildcards inside `[…]`, unvalidated; `!` stays.
    pub fn architectures(&self) -> &[String] {
        &self.architectures
    }

    /// Everything after the Tag, whitespace collapsed; empty when absent.
    /// `*` wildcards are not expanded.
    pub fn context(&self) -> &str {
        &self.context
    }

    /// 1-based line in the override file.
    pub fn line(&self) -> u32 {
        self.line
    }

    /// The `#` comment lines just above, joined with spaces. Without one, the
    /// previous override's justification when it is the same Tag in the same
    /// block.
    pub fn justification(&self) -> &str {
        &self.justification
    }
}

impl OverrideError {
    pub fn line(&self) -> u32 {
        self.line
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

/// First candidate present as a file. A symlink counts as present with no
/// content; a directory does not count (lintian's index names it with a
/// trailing slash).
fn first_present(files: &FileTree, candidates: &[PathBuf]) -> Option<(PathBuf, Vec<u8>)> {
    candidates.iter().find_map(|path| {
        let entry = files.get(path)?;
        match entry.kind() {
            EntryKind::RegularFile | EntryKind::Hardlink => {
                Some((path.clone(), files.read(entry).unwrap_or_default()))
            }
            EntryKind::Symlink => Some((path.clone(), Vec::new())),
            EntryKind::Directory
            | EntryKind::Fifo
            | EntryKind::CharDevice
            | EntryKind::BlockDevice => None,
        }
    })
}

/// `text` without a leading `word` followed by a space or a colon.
fn strip_word<'t>(text: &'t str, word: &str) -> Option<&'t str> {
    if word.is_empty() {
        return None;
    }
    let rest = text.strip_prefix(word)?;
    rest.starts_with([' ', ':']).then_some(rest)
}

/// A leading `[…]` followed by a space or a colon: the list inside and the rest.
fn strip_architectures(text: &str) -> Option<(&str, &str)> {
    let inner = text.strip_prefix('[')?;
    let end = inner.find(']')?;
    let rest = &inner[end + 1..];
    rest.starts_with([' ', ':'])
        .then_some((&inner[..end], rest))
}

#[cfg(test)]
mod tests {
    use crate::lint::file_tree::Entry;

    use super::*;

    fn parse(text: &str) -> LintianOverrides {
        LintianOverrides::parse(
            PathBuf::from("usr/share/lintian/overrides/foo"),
            text,
            "foo",
            ProcessableType::Binary,
        )
    }

    fn tags(overrides: &LintianOverrides) -> Vec<(&str, &str, u32)> {
        overrides
            .overrides()
            .iter()
            .map(|o| (o.tag(), o.context(), o.line()))
            .collect()
    }

    #[test]
    fn bare_tag_and_context() {
        let parsed = parse("some-tag\nother-tag  usr/bin/a   b\n");
        assert_eq!(
            tags(&parsed),
            [("some-tag", "", 1), ("other-tag", "usr/bin/a b", 2)]
        );
        assert!(parsed.errors().is_empty());
    }

    #[test]
    fn prefixes_need_a_colon() {
        let parsed = parse(
            "foo: a\nfoo binary: b\nbinary: c\nfoo [amd64 !i386] binary : d ctx\n[any-arm]: e\nfoo b\n",
        );
        assert_eq!(
            tags(&parsed),
            [
                ("a", "", 1),
                ("b", "", 2),
                ("c", "", 3),
                ("d", "ctx", 4),
                ("e", "", 5)
            ]
        );
        assert_eq!(parsed.overrides()[3].architectures(), ["amd64", "!i386"]);
        assert_eq!(parsed.overrides()[4].architectures(), ["any-arm"]);
        assert_eq!(
            parsed.errors(),
            [OverrideError {
                line: 6,
                message: "Expected a colon".to_string()
            }]
        );
    }

    #[test]
    fn other_package_or_type_stays_in_the_tag() {
        let parsed = parse("foobar: a\nfoo-other: b\nudeb: c\nsource: d\nfoo\n");
        assert_eq!(
            tags(&parsed),
            [
                ("foobar:", "a", 1),
                ("foo-other:", "b", 2),
                ("udeb:", "c", 3),
                ("source:", "d", 4),
                ("foo", "", 5)
            ]
        );
    }

    #[test]
    fn missing_tag_is_an_error() {
        let parsed = parse("foo:\nfoo :  \n");
        assert!(parsed.overrides().is_empty());
        assert_eq!(
            parsed.errors(),
            [
                OverrideError {
                    line: 1,
                    message: "Cannot parse line: foo:".to_string()
                },
                OverrideError {
                    line: 2,
                    message: "Cannot parse line: foo :  ".to_string()
                },
            ]
        );
    }

    #[test]
    fn comments_justify_the_next_override() {
        let parsed =
            parse("# first\n#   second   part\na\na ctx\nb\n\n# unused\n\na\n#\n# c says\nc\r\n");
        let justifications: Vec<_> = parsed
            .overrides()
            .iter()
            .map(|o| (o.tag(), o.justification(), o.line()))
            .collect();
        assert_eq!(
            justifications,
            [
                ("a", "first second part", 3),
                ("a", "first second part", 4),
                ("b", "", 5),
                ("a", "", 9),
                ("c", "c says", 12),
            ]
        );
    }

    #[test]
    fn installed_prefers_plain_over_gz() {
        let gz = {
            use std::io::Write;
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(b"foo: zipped\n").unwrap();
            encoder.finish().unwrap()
        };
        let tree = FileTree::from_entries(
            None,
            vec![Entry::regular_file(
                "usr/share/lintian/overrides/foo.gz",
                None,
                Some(gz.clone().into()),
            )],
        );
        let found = LintianOverrides::installed(&tree, "foo", ProcessableType::Binary).unwrap();
        assert_eq!(
            found.path(),
            Path::new("usr/share/lintian/overrides/foo.gz")
        );
        assert_eq!(tags(&found), [("zipped", "", 1)]);

        let tree = FileTree::from_entries(
            None,
            vec![
                Entry::regular_file("usr/share/lintian/overrides/foo.gz", None, Some(gz.into())),
                Entry::regular_file(
                    "usr/share/lintian/overrides/foo",
                    None,
                    Some(b"plain\n".to_vec().into()),
                ),
            ],
        );
        let found = LintianOverrides::installed(&tree, "foo", ProcessableType::Binary).unwrap();
        assert_eq!(found.path(), Path::new("usr/share/lintian/overrides/foo"));
        assert_eq!(tags(&found), [("plain", "", 1)]);
    }

    #[test]
    fn installed_skips_directories_and_bad_utf8() {
        let tree = FileTree::from_entries(
            None,
            vec![
                Entry::directory("usr/share/lintian/overrides/foo"),
                Entry::regular_file(
                    "usr/share/lintian/overrides/bar",
                    None,
                    Some(vec![0xff].into()),
                ),
            ],
        );
        assert!(LintianOverrides::installed(&tree, "foo", ProcessableType::Binary).is_none());
        let bar = LintianOverrides::installed(&tree, "bar", ProcessableType::Binary).unwrap();
        assert!(bar.overrides().is_empty());
        assert!(LintianOverrides::installed(&tree, "baz", ProcessableType::Binary).is_none());
    }

    #[test]
    fn udeb_type_prefix() {
        let parsed = LintianOverrides::parse(
            PathBuf::from("x"),
            "udeb: a\nbinary: b\n",
            "foo",
            ProcessableType::Udeb,
        );
        assert_eq!(tags(&parsed), [("a", "", 1), ("binary:", "b", 2)]);
    }

    #[test]
    fn source_prefers_debian_source_directory() {
        let tree = FileTree::from_entries(
            None,
            vec![
                Entry::regular_file(
                    "debian/source.lintian-overrides",
                    None,
                    Some(b"old\n".to_vec().into()),
                ),
                Entry::regular_file(
                    "debian/source/lintian-overrides",
                    None,
                    Some(b"src source: new\n".to_vec().into()),
                ),
            ],
        );
        let found = LintianOverrides::source(&tree, "src").unwrap();
        assert_eq!(found.path(), Path::new("debian/source/lintian-overrides"));
        assert_eq!(tags(&found), [("new", "", 1)]);

        let tree = FileTree::from_entries(
            None,
            vec![Entry::regular_file(
                "debian/source.lintian-overrides",
                None,
                Some(b"old\n".to_vec().into()),
            )],
        );
        let found = LintianOverrides::source(&tree, "src").unwrap();
        assert_eq!(found.path(), Path::new("debian/source.lintian-overrides"));
    }
}
