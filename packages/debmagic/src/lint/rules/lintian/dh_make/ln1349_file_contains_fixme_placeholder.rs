use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::{Severity, SourceRule};

pub struct FileContainsFixmePlaceholder;

declare_rule! {
    /// A packaging file still has a `FIXME` or `FIX_ME` placeholder.
    ///
    /// Checks `debian/README.source`, `debian/copyright`, `debian/rules` and
    /// `debian/control` (regular files and hardlinks only). Each line is
    /// reported once, with its first placeholder not directly inside double
    /// quotes (`"FIXME"` is skipped, as lintian's lookarounds do). Extra is the
    /// placeholder as written; the line is the Location. Reported at info
    /// severity.
    FileContainsFixmePlaceholder,
    code = "LN1349",
    tag = "file-contains-fixme-placeholder",
    default_selected = false,
    experimental = false,
}

const CHECKED: [&str; 4] = [
    "debian/README.source",
    "debian/copyright",
    "debian/rules",
    "debian/control",
];

impl SourceRule for FileContainsFixmePlaceholder {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let targets: Vec<(PathBuf, Vec<u8>)> = files
            .entries()
            .iter()
            .filter(|entry| matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink))
            .filter(|entry| {
                entry
                    .path()
                    .to_str()
                    .is_some_and(|path| CHECKED.contains(&path))
            })
            .filter_map(|entry| Some((entry.path().to_path_buf(), files.read(entry).ok()?)))
            .collect();
        let mut findings = Vec::new();
        for (path, bytes) in targets {
            for (index, line) in bytes.split(|&byte| byte == b'\n').enumerate() {
                let Some(placeholder) = placeholder(line) else {
                    continue;
                };
                findings.push((path.clone(), index + 1, placeholder));
            }
        }
        for (path, line, placeholder) in findings {
            ctx.diagnostic(
                Diagnostic::warn(placeholder)
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: u32::try_from(line).ok(),
                        column: None,
                    }),
            );
        }
    }
}

/// The first `FIXME` or `FIX_ME` on the line that is not directly inside
/// double quotes: lintian's `(?<!")(FIX_?ME)(?!")`.
fn placeholder(line: &[u8]) -> Option<String> {
    for start in 0..line.len() {
        let rest = &line[start..];
        let len = if rest.starts_with(b"FIX_ME") {
            6
        } else if rest.starts_with(b"FIXME") {
            5
        } else {
            continue;
        };
        let quoted_before = start > 0 && line[start - 1] == b'"';
        let quoted_after = line.get(start + len) == Some(&b'"');
        if quoted_before || quoted_after {
            continue;
        }
        return Some(String::from_utf8_lossy(&rest[..len]).into_owned());
    }
    None
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\nMaintainer: Example <ex@example.com>\n\n";

    fn tree(path: &str, content: &str) -> Vec<(String, TestFile)> {
        vec![
            ("debian/control".to_string(), TestFile::new(CONTROL)),
            (path.to_string(), TestFile::new(content)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            tree("debian/copyright", "Comment: \"FIXME\" is quoted\n"),
            tree("debian/changelog", "FIXME in a file not checked\n"),
            tree("debian/README.source", "Nothing to see here.\n"),
        ];
        let fail = vec![tree(
            "debian/rules",
            "#!/usr/bin/make -f\n# FIX_ME: write this\n",
        )];
        SourceTreeTester::new(FileContainsFixmePlaceholder, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            "debian/copyright",
            "License: MIT\n",
        ))];
        let fail = vec![SourcePackageCase::from(tree(
            "debian/control",
            "Source: example\nDescription: FIXME\n",
        ))];
        SourcePackageTester::new(FileContainsFixmePlaceholder, pass, fail).test_and_snapshot();
    }
}
