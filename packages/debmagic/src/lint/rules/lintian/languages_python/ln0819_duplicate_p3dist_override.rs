use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::SourceRule;

pub struct DuplicateP3distOverride;

declare_rule! {
    /// `debian/py3dist-overrides` names the same Python import on more than one
    /// line.
    ///
    /// Only the first line for an import is used by `dh_python3`; the others
    /// are discarded. Comment lines and blank lines are skipped. The first
    /// whitespace-delimited word of each other line is the import. Each import
    /// that occurs more than once is reported once, in the order it first
    /// appears, with the import as extra. The File tree is the Source tree or
    /// Source package Patched files. A symlinked override file is not read.
    DuplicateP3distOverride,
    code = "LN0819",
    tag = "duplicate-p3dist-override",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DuplicateP3distOverride {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let duplicates = duplicates_in(files);
        for identifier in duplicates {
            ctx.diagnostic(Diagnostic::error(identifier));
        }
    }
}

fn duplicates_in(files: &FileTree) -> Vec<String> {
    let Some(entry) = files.get("debian/py3dist-overrides") else {
        return Vec::new();
    };
    if !matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink) {
        return Vec::new();
    }
    let Ok(bytes) = files.read(entry) else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&bytes);
    duplicates(&text)
}

fn duplicates(text: &str) -> Vec<String> {
    let identifiers: Vec<&str> = text
        .lines()
        .map(str::trim_start)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_whitespace().next())
        .collect();

    let mut unique: Vec<&str> = Vec::new();
    for identifier in &identifiers {
        if !unique.contains(identifier) {
            unique.push(identifier);
        }
    }

    unique
        .into_iter()
        .filter(|identifier| {
            identifiers
                .iter()
                .filter(|other| *other == identifier)
                .count()
                > 1
        })
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    fn tree(overrides: &str) -> Vec<(String, TestFile)> {
        vec![
            ("debian/control".to_string(), TestFile::from(CONTROL)),
            (
                "debian/py3dist-overrides".to_string(),
                TestFile::new(overrides),
            ),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/control".to_string(), TestFile::from(CONTROL))],
            tree(""),
            tree("gi.repository.Gst gir1.2-gst-plugins-base-1.0\n"),
            tree("gi.repository.Gst gir1.2-gst-plugins-base-1.0\nfoo bar\n"),
            // A double dependency on one line is kept.
            tree("gi.repository.Gst gir1.2-gst-plugins-base-1.0, gir1.2-gstreamer-1.0\n"),
            // Comments and blank lines do not count.
            tree("# gi.repository.Gst\n\n  # gi.repository.Gst x\ngi.repository.Gst a\n"),
            // The same word later on a line is not an identifier.
            tree("gi.repository.Gst a\nfoo gi.repository.Gst\n"),
        ];
        let fail = vec![
            tree("gi.repository.Gst a\ngi.repository.Gst b\n"),
            // Leading whitespace is ignored.
            tree("  gi.repository.Gst a\n\tgi.repository.Gst b\n"),
            // Reported once, in order of first appearance.
            tree("foo a\nbar a\nfoo b\nbar c\nfoo c\n"),
            tree("foo a\nfoo b\nfoo c\n"),
        ];
        SourceTreeTester::new(DuplicateP3distOverride, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            SourcePackageCase::from(vec![("debian/control", TestFile::from(CONTROL))]),
            SourcePackageCase::from(vec![(
                "debian/py3dist-overrides",
                TestFile::new("gi.repository.Gst a\n"),
            )]),
        ];
        let fail = vec![SourcePackageCase::from(vec![
            ("debian/control", TestFile::from(CONTROL)),
            (
                "debian/py3dist-overrides",
                TestFile::new("gi.repository.Gst a\ngi.repository.Gst b\n"),
            ),
        ])];
        SourcePackageTester::new(DuplicateP3distOverride, pass, fail).test_and_snapshot();
    }
}
