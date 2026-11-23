use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::series_patches::{listed_patch, plain_series};

pub struct PatchModifyingDebianFiles;

declare_rule! {
    /// A patch in the plain Quilt series that modifies a file under `debian/`.
    ///
    /// Runs when Format is `3.x (quilt)` (Dsc, else `debian/source/format`)
    /// or `Build-Depends-All` of Debian control satisfies `quilt`, for each
    /// patch that `debian/patches/series` lists and that resolves to an open
    /// regular file under `debian/patches/`. Commented-out and vendor series
    /// entries do not count. The patch is read as a unified diff: each
    /// `--- old` line followed by `+++ new` names a file (the new name, or the
    /// old one for `/dev/null`), and the first path component is stripped as
    /// `lsdiff --strip=1` does. Each stripped name that starts with `debian/`
    /// or `./debian/` is reported once, with that name as Extra and the
    /// resolved patch as the Location (no line). Applies to the Source tree
    /// and Source package.
    PatchModifyingDebianFiles,
    code = "LN1770",
    tag = "patch-modifying-debian-files",
    default_selected = true,
    experimental = false,
}

impl SourceRule for PatchModifyingDebianFiles {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some((_, names)) = plain_series(ctx) else {
            return;
        };
        let mut hints = Vec::new();
        if let Some(files) = ctx.files() {
            for name in &names {
                let Some(patch) = listed_patch(files, name) else {
                    continue;
                };
                let Ok(bytes) = files.read(patch) else {
                    continue;
                };
                for debian_file in debian_files_in_diff(&bytes) {
                    hints.push((patch.path().to_path_buf(), debian_file));
                }
            }
        }
        for (path, debian_file) in hints {
            ctx.diagnostic(Diagnostic::error(debian_file).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// The names a unified diff modifies that start with `debian/` or
/// `./debian/`, after the first path component is stripped, in diff order.
/// Context diffs, and git headers without `---`/`+++` lines (pure renames,
/// mode changes), are not recognised.
fn debian_files_in_diff(diff: &[u8]) -> Vec<String> {
    let lines: Vec<&[u8]> = diff.split(|&byte| byte == b'\n').collect();
    let mut names = Vec::new();
    for pair in lines.windows(2) {
        let (Some(old), Some(new)) = (pair[0].strip_prefix(b"--- "), pair[1].strip_prefix(b"+++ "))
        else {
            continue;
        };
        let name = if header_name(new) == b"/dev/null" {
            header_name(old)
        } else {
            header_name(new)
        };
        let Some(stripped) = strip_first_component(name) else {
            continue;
        };
        let text = String::from_utf8_lossy(stripped).into_owned();
        if text.starts_with("debian/") || text.starts_with("./debian/") {
            names.push(text);
        }
    }
    names
}

/// A `---`/`+++` header value: up to a tab (the timestamp separator), with a
/// trailing carriage return removed.
fn header_name(value: &[u8]) -> &[u8] {
    let name = value
        .split(|&byte| byte == b'\t')
        .next()
        .unwrap_or_default();
    name.strip_suffix(b"\r").unwrap_or(name)
}

/// `lsdiff --strip=1`: drop the first `/`-separated component. `None` when
/// there is no `/` to drop.
fn strip_first_component(name: &[u8]) -> Option<&[u8]> {
    let index = name.iter().position(|&byte| byte == b'/')?;
    let rest = &name[index + 1..];
    (!rest.is_empty()).then_some(rest)
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "\
Source: example
Maintainer: Maintainer <m@example.com>

Package: example
Architecture: all
Description: example
 extra
";

    const CONTROL_QUILT: &str = "\
Source: example
Maintainer: Maintainer <m@example.com>
Build-Depends: debhelper-compat (= 13), quilt

Package: example
Architecture: all
Description: example
 extra
";

    const DEBIAN_CONTROL_DIFF: &str = "\
--- a/debian/control
+++ b/debian/control
@@ -1 +1 @@
-old
+new
";

    const SOURCE_DIFF: &str = "\
--- a/src/main.c
+++ b/src/main.c
@@ -1 +1 @@
-old
+new
";

    fn tree(
        format: Option<&str>,
        control: &str,
        extra: &[(&str, TestFile)],
    ) -> Vec<(String, TestFile)> {
        let mut files = vec![("debian/control".to_string(), TestFile::new(control))];
        if let Some(format) = format {
            files.push((
                "debian/source/format".to_string(),
                TestFile::new(format!("{format}\n")),
            ));
        }
        for (path, file) in extra {
            files.push(((*path).to_string(), file.clone()));
        }
        files
    }

    fn quilt(extra: &[(&str, TestFile)]) -> Vec<(String, TestFile)> {
        tree(Some("3.0 (quilt)"), CONTROL, extra)
    }

    fn file(content: &str) -> TestFile {
        TestFile::new(content)
    }

    #[test]
    fn source() {
        let pass = vec![
            quilt(&[]),
            // Patches that modify other trees.
            quilt(&[
                ("debian/patches/series", file("src.patch\n")),
                ("debian/patches/src.patch", file(SOURCE_DIFF)),
            ]),
            // `debian/` after stripping one component is not `debian/`:
            // the diff names `debian/control` with no leading directory.
            quilt(&[
                ("debian/patches/series", file("bare.patch\n")),
                (
                    "debian/patches/bare.patch",
                    file("--- debian/control\n+++ debian/control\n"),
                ),
            ]),
            // Commented-out and vendor series entries are not checked.
            quilt(&[
                ("debian/patches/series", file("# off.patch\n")),
                ("debian/patches/vendor.series", file("vendor.patch\n")),
                ("debian/patches/off.patch", file(DEBIAN_CONTROL_DIFF)),
                ("debian/patches/vendor.patch", file(DEBIAN_CONTROL_DIFF)),
            ]),
            // A patch not listed in the series is not checked.
            quilt(&[
                ("debian/patches/series", file("")),
                ("debian/patches/unlisted.patch", file(DEBIAN_CONTROL_DIFF)),
            ]),
            // Not quilt: Format 1.0 without a quilt build dependency.
            tree(
                Some("1.0"),
                CONTROL,
                &[
                    ("debian/patches/series", file("a.patch\n")),
                    ("debian/patches/a.patch", file(DEBIAN_CONTROL_DIFF)),
                ],
            ),
            // Format 1.0 with a quilt build dependency is read, but a
            // patch that does not touch debian/ passes.
            tree(
                Some("1.0"),
                CONTROL_QUILT,
                &[
                    ("debian/patches/series", file("a.patch\n")),
                    ("debian/patches/a.patch", file(SOURCE_DIFF)),
                ],
            ),
            // Series missing, or a dangling series symlink.
            quilt(&[("debian/patches/a.patch", file(DEBIAN_CONTROL_DIFF))]),
            quilt(&[
                ("debian/patches/series", TestFile::symlink("missing")),
                ("debian/patches/a.patch", file(DEBIAN_CONTROL_DIFF)),
            ]),
            // A listed patch that is missing, or a directory.
            quilt(&[
                ("debian/patches/series", file("gone.patch\nsub\n")),
                ("debian/patches/sub", TestFile::directory()),
            ]),
            // A patch symlink that dangles, or escapes the tree.
            quilt(&[
                (
                    "debian/patches/series",
                    file("dangling.patch\nescape.patch\n"),
                ),
                (
                    "debian/patches/dangling.patch",
                    TestFile::symlink("missing"),
                ),
                (
                    "debian/patches/escape.patch",
                    TestFile::symlink("../../../outside.patch"),
                ),
            ]),
            // Only the `debian/` names are reported, and an `a/` prefix is
            // the one stripped.
            quilt(&[
                ("debian/patches/series", file("a.patch\n")),
                (
                    "debian/patches/a.patch",
                    file("--- a/src/debian/x\n+++ b/src/debian/x\n"),
                ),
            ]),
            // Non-UTF-8 content around the headers does not stop the scan.
            quilt(&[
                ("debian/patches/series", file("bytes.patch\n")),
                (
                    "debian/patches/bytes.patch",
                    TestFile::from_bytes(b"\xff\xfe junk\n--- a/src/x\n+++ b/src/x\n".to_vec()),
                ),
            ]),
        ];
        let fail = vec![
            // The patch modifies debian/control.
            quilt(&[
                ("debian/patches/series", file("a.patch\n")),
                ("debian/patches/a.patch", file(DEBIAN_CONTROL_DIFF)),
            ]),
            // Several debian files in one patch, with `/dev/null` for a new
            // file, and a `./debian/` name; the name is the patch's new name.
            quilt(&[
                ("debian/patches/series", file("many.patch\n")),
                (
                    "debian/patches/many.patch",
                    file(
                        "--- a/debian/rules\n+++ b/debian/rules\n@@ -1 +1 @@\n\
                         --- a/src/x\n+++ b/src/x\n\
                         --- /dev/null\n+++ b/debian/new\t2020-01-01\n\
                         --- a/./debian/gone\n+++ /dev/null\n",
                    ),
                ),
            ]),
            // Format 1.0 with a quilt build dependency is quilt.
            tree(
                Some("1.0"),
                CONTROL_QUILT,
                &[
                    ("debian/patches/series", file("a.patch\n")),
                    ("debian/patches/a.patch", file(DEBIAN_CONTROL_DIFF)),
                ],
            ),
            // Series entry with options, and a patch reached through a
            // symlink inside the tree: the Location is the resolved patch.
            quilt(&[
                ("debian/patches/series", file("link.patch -p1\n")),
                ("debian/patches/real.patch", file(DEBIAN_CONTROL_DIFF)),
                ("debian/patches/link.patch", TestFile::symlink("real.patch")),
            ]),
            // A hardlinked patch is open.
            quilt(&[
                ("debian/patches/series", file("hard-copy.patch\n")),
                ("debian/patches/hard.patch", file(DEBIAN_CONTROL_DIFF)),
                (
                    "debian/patches/hard-copy.patch",
                    TestFile::new(DEBIAN_CONTROL_DIFF).hardlink_to("debian/patches/hard.patch"),
                ),
            ]),
            // Series reached through a symlink inside the tree.
            tree(
                Some("3.0 (quilt)"),
                CONTROL,
                &[
                    (
                        "debian/patches/series",
                        TestFile::symlink("../quilt-series"),
                    ),
                    ("debian/quilt-series", file("a.patch\n")),
                    ("debian/patches/a.patch", file(DEBIAN_CONTROL_DIFF)),
                ],
            ),
        ];
        SourceTreeTester::new(PatchModifyingDebianFiles, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let files = quilt(&[
            ("debian/patches/series", file("a.patch\n")),
            ("debian/patches/a.patch", file(DEBIAN_CONTROL_DIFF)),
        ]);
        let pass = vec![
            SourcePackageCase::from(files.clone())
                .dsc(Dsc::parse("Format: 1.0\nVersion: 1.0-1\nSource: example\n").expect("dsc")),
        ];
        let fail = vec![SourcePackageCase::from(files).dsc(
            Dsc::parse("Format: 3.0 (quilt)\nVersion: 1.0-1\nSource: example\n").expect("dsc"),
        )];
        SourcePackageTester::new(PatchModifyingDebianFiles, pass, fail).test_and_snapshot();
    }

    #[test]
    fn diff_names() {
        assert_eq!(
            debian_files_in_diff(b"--- a/debian/x\n+++ b/debian/x\n"),
            ["debian/x"]
        );
        assert_eq!(
            debian_files_in_diff(b"--- a/./debian/x\n+++ b/./debian/x\n"),
            ["./debian/x"]
        );
        // The new name wins unless it is /dev/null.
        assert_eq!(
            debian_files_in_diff(b"--- a/debian/x\n+++ /dev/null\n"),
            ["debian/x"]
        );
        // No header pair, no name.
        assert!(debian_files_in_diff(b"+++ b/debian/x\n--- a/debian/x\n").is_empty());
        assert!(debian_files_in_diff(b"--- a/debian/x\n").is_empty());
        // No component to strip.
        assert!(debian_files_in_diff(b"--- debian\n+++ debian\n").is_empty());
    }
}
