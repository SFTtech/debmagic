use std::path::Path;

use regex::Regex;
use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::SourceRule;

pub struct Format30ButDebianChangesPatch;

declare_rule! {
    /// A 3.0 (quilt) source ships `debian/patches/debian-changes-$version`.
    ///
    /// Format comes from the Dsc when there is one, otherwise the first line
    /// of `debian/source/format`. The version is the Dsc version, otherwise
    /// the newest changelog entry. The patch must be a regular file or
    /// hardlink. `debian/source/patch-header` as a file suppresses the Tag.
    /// Extra is empty. The Location is the patch. Applies to the Source tree
    /// and Source package.
    Format30ButDebianChangesPatch,
    code = "LN0396",
    tag = "format-3.0-but-debian-changes-patch",
    default_selected = true,
    experimental = false,
}

static QUILT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"3\.\d+ \(quilt\)").expect("regex"));

impl SourceRule for Format30ButDebianChangesPatch {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if !QUILT.is_match(&source_format(ctx)) {
            return;
        }
        let Some(version) = package_version(ctx) else {
            return;
        };
        let Some(files) = ctx.files() else {
            return;
        };
        if is_file(files, Path::new("debian/source/patch-header")) {
            return;
        }
        let path = format!("debian/patches/debian-changes-{version}");
        if !is_file(files, Path::new(&path)) {
            return;
        }
        ctx.diagnostic(Diagnostic::warn("").with_location(Location {
            path: path.into(),
            line: None,
            column: None,
        }));
    }
}

fn source_format(ctx: &mut SourceContext<'_>) -> String {
    if let Some(format) = ctx.dsc().and_then(|dsc| dsc.fields().get("Format")) {
        return format.trim().to_string();
    }
    let Some(files) = ctx.files() else {
        return String::new();
    };
    let Some(entry) = files.get(Path::new("debian/source/format")) else {
        return String::new();
    };
    if !is_file_entry(entry.kind()) {
        return String::new();
    }
    let Ok(bytes) = files.read(entry) else {
        return String::new();
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return String::new();
    };
    text.lines().next().unwrap_or("").trim().to_string()
}

fn package_version(ctx: &SourceContext<'_>) -> Option<String> {
    if let Some(version) = ctx.dsc().and_then(|dsc| dsc.fields().get("Version")) {
        let version = version.trim();
        if !version.is_empty() {
            return Some(version.to_string());
        }
    }
    ctx.debian_changelog()?
        .entries()
        .first()
        .map(|entry| entry.version().to_string())
        .filter(|version| !version.is_empty())
}

fn is_file(files: &FileTree, path: &Path) -> bool {
    files
        .get(path)
        .is_some_and(|entry| is_file_entry(entry.kind()))
}

fn is_file_entry(kind: EntryKind) -> bool {
    matches!(kind, EntryKind::RegularFile | EntryKind::Hardlink)
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn changelog(version: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * Lintian Test Suite.\n\n -- Maintainer <m@example.com>  Tue, 31 Mar 2026 17:46:22 +0200\n"
        )
    }

    fn tree(format: &str, version: &str, extra: &[(&str, &str)]) -> Vec<(String, TestFile)> {
        let mut files = vec![
            (
                "debian/source/format".to_string(),
                TestFile::new(format!("{format}\n")),
            ),
            (
                "debian/changelog".to_string(),
                TestFile::new(changelog(version)),
            ),
            ("debian/control".to_string(), TestFile::new(CONTROL)),
        ];
        for (path, body) in extra {
            files.push(((*path).to_string(), TestFile::new(*body)));
        }
        files
    }

    const CONTROL: &str = "\
Source: example
Maintainer: Maintainer <m@example.com>

Package: example
Architecture: all
Description: example
 extra
";

    #[test]
    fn test() {
        let pass = vec![
            vec![("README".to_string(), TestFile::new("no packaging"))],
            tree("3.0 (quilt)", "1.0-1", &[]),
            tree(
                "3.0 (native)",
                "1.0",
                &[("debian/patches/debian-changes-1.0", "diff")],
            ),
            tree(
                "3.0 (quilt)",
                "1.0-1",
                &[
                    ("debian/patches/debian-changes-1.0-1", "diff"),
                    ("debian/source/patch-header", "explained\n"),
                ],
            ),
            tree(
                "3.0 (quilt)",
                "1.0-1",
                &[("debian/patches/series", "debian-changes-1.0-1\n")],
            ),
        ];
        let fail = vec![
            tree(
                "3.0 (quilt)",
                "1.0-1",
                &[("debian/patches/debian-changes-1.0-1", "diff\n")],
            ),
            tree(
                "3.1 (quilt)",
                "2:1.0-1",
                &[("debian/patches/debian-changes-2:1.0-1", "diff\n")],
            ),
        ];
        SourceTreeTester::new(Format30ButDebianChangesPatch, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let files = tree(
            "1.0",
            "1.0-1",
            &[("debian/patches/debian-changes-9.9-1", "diff\n")],
        );
        let pass = vec![SourcePackageCase::from(files.clone())];
        let fail = vec![SourcePackageCase::from(files).dsc(
            Dsc::parse("Format: 3.0 (quilt)\nVersion: 9.9-1\nSource: example\n").expect("dsc"),
        )];
        SourcePackageTester::new(Format30ButDebianChangesPatch, pass, fail).test_and_snapshot();
    }
}
