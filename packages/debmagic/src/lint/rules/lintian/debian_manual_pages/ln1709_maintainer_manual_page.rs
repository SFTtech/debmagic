use std::path::Path;

use crate::declare_rule;
use crate::lint::context::{SourceContext, resolve_symlinks};
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::{Severity, SourceRule};
use crate::lint::rules::lintian::fields_version::is_native;

pub struct MaintainerManualPage;

declare_rule! {
    /// A non-native source package keeps a manual page in `debian/`, outside
    /// `debian/patches/`, whose command is not packaged in the same `debian/`
    /// tree. The manual page should be forwarded upstream.
    ///
    /// The page is a regular file or hardlink whose name ends in `.N` (one
    /// digit). The command is the name without that suffix. The page is not
    /// reported when some file below `debian/` (patches included) has that
    /// command as its basename, as in `debian/bin/<command>`. Native packages
    /// are skipped. Reported at pedantic severity with the page path as the
    /// Location and an empty extra.
    MaintainerManualPage,
    code = "LN1709",
    tag = "maintainer-manual-page",
    default_selected = false,
    experimental = false,
}

impl SourceRule for MaintainerManualPage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if is_native(ctx) {
            return;
        }
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(debian) = files.get("debian") else {
            return;
        };
        let Some(debian) = resolve_symlinks(files, debian) else {
            return;
        };
        let prefix = debian.path().to_path_buf();
        let below: Vec<&Path> = files
            .entries()
            .iter()
            .filter(|entry| entry.path().starts_with(&prefix) && entry.path() != prefix)
            .filter(|entry| matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink))
            .map(|entry| entry.path())
            .collect();
        let patches = Path::new("debian/patches");
        let pages: Vec<&Path> = below
            .iter()
            .copied()
            .filter(|path| !path.starts_with(patches))
            .filter(|path| basename(path).is_some_and(is_manual_page_name))
            .collect();
        let mut hits = Vec::new();
        for page in pages {
            let Some(name) = basename(page) else {
                continue;
            };
            let command = strip_section(name);
            if !below.iter().any(|path| basename(path) == Some(command)) {
                hits.push(page.to_path_buf());
            }
        }
        for path in hits {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

fn basename(path: &Path) -> Option<&str> {
    path.file_name().and_then(|name| name.to_str())
}

/// Lintian's `\.\d$`: a name ending in a dot and one digit.
fn is_manual_page_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() >= 2 && bytes[bytes.len() - 2] == b'.' && bytes[bytes.len() - 1].is_ascii_digit()
}

/// The name without its one-digit section suffix (`s/ [.] \d $//x`).
fn strip_section(name: &str) -> &str {
    name.strip_suffix(|digit: char| digit.is_ascii_digit())
        .and_then(|rest| rest.strip_suffix('.'))
        .unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn tree(format: &str, extra: &[(&'static str, TestFile)]) -> Vec<(String, TestFile)> {
        let mut files = vec![
            ("debian".to_string(), TestFile::directory()),
            (
                "debian/source/format".to_string(),
                TestFile::new(format!("{format}\n")),
            ),
        ];
        for (path, file) in extra {
            files.push(((*path).to_string(), file.clone()));
        }
        files
    }

    #[test]
    fn source() {
        let pass = vec![
            tree(
                "3.0 (native)",
                &[("debian/example.1", TestFile::new(".TH"))],
            ),
            // The command is packaged under debian/ (here in a subdirectory).
            tree(
                "3.0 (quilt)",
                &[
                    ("debian/example.1", TestFile::new(".TH")),
                    ("debian/bin/example", TestFile::new("x")),
                ],
            ),
            // A patch named like the command counts as packaging it.
            tree(
                "3.0 (quilt)",
                &[
                    ("debian/tool.8", TestFile::new(".TH")),
                    ("debian/patches/tool", TestFile::new("x")),
                ],
            ),
            // Pages in debian/patches/ and names without a section are not checked.
            tree(
                "3.0 (quilt)",
                &[
                    ("debian/patches/x.1", TestFile::new(".TH")),
                    ("debian/example.man", TestFile::new(".TH")),
                ],
            ),
        ];
        let fail = vec![
            tree(
                "3.0 (quilt)",
                &[("debian/not-forwarded.1", TestFile::new(".TH"))],
            ),
            // A patch file of the command's name does not package the page's command.
            tree(
                "3.0 (quilt)",
                &[
                    ("debian/tool.8", TestFile::new(".TH")),
                    ("debian/patches/tool.8", TestFile::new("x")),
                ],
            ),
        ];
        SourceTreeTester::new(MaintainerManualPage, pass, fail).test_and_snapshot();
    }
}
