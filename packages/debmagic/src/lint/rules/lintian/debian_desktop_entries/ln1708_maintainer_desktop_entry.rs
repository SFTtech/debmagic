use std::path::Path;

use crate::declare_rule;
use crate::lint::context::{SourceContext, resolve_symlinks};
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::{Severity, SourceRule};
use crate::lint::rules::lintian::fields_version::is_native;

pub struct MaintainerDesktopEntry;

declare_rule! {
    /// A non-native source package keeps a desktop entry in `debian/`, outside
    /// `debian/patches/`. The desktop entry should be forwarded upstream.
    ///
    /// Native packages are skipped (see `fields_version::is_native`). The
    /// `debian` directory is followed through symbolic links inside the tree.
    /// Every regular file or hardlink below it whose name ends in `.desktop`
    /// is reported, with its path as the Location and an empty extra. Reported
    /// at pedantic severity. Applies to the Source tree and Source package.
    MaintainerDesktopEntry,
    code = "LN1708",
    tag = "maintainer-desktop-entry",
    default_selected = false,
    experimental = false,
}

impl SourceRule for MaintainerDesktopEntry {
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
        let patches = Path::new("debian/patches");
        let hits: Vec<Location> = files
            .entries()
            .iter()
            .filter(|entry| entry.path().starts_with(&prefix) && entry.path() != prefix)
            .filter(|entry| matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink))
            .filter(|entry| !entry.path().starts_with(patches))
            .filter(|entry| {
                entry
                    .path()
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(".desktop"))
            })
            .map(|entry| Location {
                path: entry.path().to_path_buf(),
                line: None,
                column: None,
            })
            .collect();
        for location in hits {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Pedantic)
                    .with_location(location),
            );
        }
    }
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
            // Native package: not checked.
            tree(
                "3.0 (native)",
                &[("debian/example.desktop", TestFile::new("[Desktop Entry]\n"))],
            ),
            // Desktop entries under debian/patches/ and other names are not reported.
            tree(
                "3.0 (quilt)",
                &[
                    ("debian/patches/example.desktop", TestFile::new("x")),
                    ("debian/example.desktop.in", TestFile::new("x")),
                ],
            ),
        ];
        let fail = vec![
            tree(
                "3.0 (quilt)",
                &[("debian/example.desktop", TestFile::new("[Desktop Entry]\n"))],
            ),
            tree(
                "3.0 (quilt)",
                &[
                    ("debian/a/b.desktop", TestFile::new("x")),
                    ("debian/c.desktop", TestFile::new("x")),
                ],
            ),
        ];
        SourceTreeTester::new(MaintainerDesktopEntry, pass, fail).test_and_snapshot();
    }
}
