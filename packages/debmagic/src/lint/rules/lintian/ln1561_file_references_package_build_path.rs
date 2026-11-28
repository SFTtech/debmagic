use crate::declare_rule;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::group_context::GroupContext;
use crate::lint::rule::{GroupRule, Severity};

pub struct FileReferencesPackageBuildPath;

declare_rule! {
    /// A regular file of an installable of the Group (deb or udeb) contains
    /// the build path, the `Build-Path` field of the Group's `.buildinfo`. The
    /// hint is located at that file, has no extra and is info severity; the
    /// Tag is not selected by default, as in lintian.
    ///
    /// A Group Rule: the `Build-Path` is in the `.buildinfo`, and the Tag only
    /// appears when that field is there (`buildinfo=+path` in
    /// `DEB_BUILD_OPTIONS`). Without a `.buildinfo`, or with an empty
    /// `Build-Path`, nothing is reported. The match is a plain byte search of
    /// the file's contents, as lintian's `bytes_match` on the quoted path.
    /// Symbolic links and other non-regular entries are not read; hardlinks
    /// are. An installable's Check in lintian, so it also runs on a lone
    /// `.deb`, where the `.buildinfo` is what decides.
    FileReferencesPackageBuildPath,
    code = "LN1561",
    tag = "file-references-package-build-path",
    default_selected = false,
    experimental = false,
}

impl GroupRule for FileReferencesPackageBuildPath {
    fn runs_without_dsc(&self) -> bool {
        true
    }

    fn run(&self, ctx: &mut GroupContext<'_>) {
        let Some(build_path) = ctx
            .buildinfo()
            .and_then(|buildinfo| buildinfo.fields().get("Build-Path"))
            .filter(|path| !path.is_empty())
        else {
            return;
        };
        let needle = build_path.as_bytes();
        for installable in ctx.installables() {
            let Some(files) = ctx.data_files(&installable) else {
                continue;
            };
            for entry in files.entries() {
                if !entry.is_file() {
                    continue;
                }
                let Ok(bytes) = files.read(entry) else {
                    continue;
                };
                if !contains(&bytes, needle) {
                    continue;
                }
                ctx.installable_diagnostic(
                    &installable,
                    Diagnostic::warn("")
                        .with_severity(Severity::Info)
                        .with_location(Location {
                            path: entry.path().to_path_buf(),
                            line: None,
                            column: None,
                        }),
                );
            }
        }
    }
}

/// Whether `needle` occurs in `haystack`.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty()
        || haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{GroupCase, GroupTester, TestCase, TestFile};

    use super::*;

    const DSC: &str = "Source: example\nVersion: 1.0\n";
    const BUILDINFO: &str = "Format: 1.0\nSource: example\nBinary: example\nArchitecture: all\nVersion: 1.0\nBuild-Path: /build/example-1.0\n";

    fn group(files: Vec<(&str, &str)>) -> GroupCase {
        GroupCase::new(DSC)
            .buildinfo(BUILDINFO)
            .installable("example", "")
            .files(TestCase::from(files))
    }

    #[test]
    fn group_rule() {
        let pass = vec![
            // No `.buildinfo`, so no `Build-Path`.
            GroupCase::new(DSC)
                .installable("example", "")
                .files(TestCase::from(vec![(
                    "usr/share/doc/example/build-path",
                    "/build/example-1.0\n",
                )])),
            // A `.buildinfo` without `Build-Path`.
            GroupCase::new(DSC)
                .buildinfo("Format: 1.0\nSource: example\nVersion: 1.0\n")
                .installable("example", "")
                .files(TestCase::from(vec![(
                    "usr/share/doc/example/build-path",
                    "/build/example-1.0\n",
                )])),
            // A lone .deb without a .buildinfo: no Build-Path to look for.
            GroupCase::without_dsc()
                .installable("example", "")
                .files(TestCase::from(vec![(
                    "usr/share/doc/example/build-path",
                    "/build/example-1.0\n",
                )])),
            // The path is not in the files.
            group(vec![("usr/share/doc/example/README", "/build/other\n")]),
            // A symbolic link to the path is not read.
            GroupCase::new(DSC)
                .buildinfo(BUILDINFO)
                .installable("example", "")
                .files(TestCase::from(vec![(
                    "usr/share/doc/example/build-path",
                    TestFile::symlink("/build/example-1.0"),
                )])),
        ];
        let fail = vec![
            group(vec![(
                "usr/share/doc/example/build-path",
                "/build/example-1.0\n",
            )]),
            // A match inside a larger file.
            group(vec![(
                "usr/bin/example",
                "prefix /build/example-1.0 suffix",
            )]),
            // A udeb is checked too.
            GroupCase::new(DSC)
                .buildinfo(BUILDINFO)
                .udeb("example-udeb", "")
                .files(TestCase::from(vec![(
                    "usr/lib/example/build-path",
                    "/build/example-1.0",
                )])),
        ];
        GroupTester::new(FileReferencesPackageBuildPath, pass, fail).test_and_snapshot();
    }
}
