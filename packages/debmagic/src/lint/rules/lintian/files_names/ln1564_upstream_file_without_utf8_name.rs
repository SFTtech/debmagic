use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::{Severity, SourceRule};
use crate::lint::rules::lintian::debian_changelog::source::package_is_native;

pub struct UpstreamFileWithoutUtf8Name;

declare_rule! {
    /// A non-native Source package has an Orig file whose name is not UTF-8.
    ///
    /// Only for non-native packages (lintian's `!native`). Each Orig files
    /// entry whose path is not valid UTF-8, directories included (with the
    /// trailing slash lintian gives them). The extra is empty and the Location
    /// is the name, printed with every byte read as Latin-1 as lintian
    /// does, so the bytes the file system holds stay visible. Reported in path
    /// order.
    ///
    /// A Source tree has no Orig files, so only the Source package is
    /// checked. The Patched files copy of the same names is
    /// `patched-file-without-utf8-name`.
    UpstreamFileWithoutUtf8Name,
    code = "LN1564",
    tag = "upstream-file-without-utf8-name",
    default_selected = false,
    experimental = false,
}

impl SourceRule for UpstreamFileWithoutUtf8Name {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if package_is_native(ctx) {
            return;
        }
        let Some(orig) = ctx.orig_files() else {
            return;
        };
        let mut found: Vec<PathBuf> = orig
            .entries()
            .iter()
            .filter(|entry| entry.path().to_str().is_none())
            .map(|entry| {
                let mut name = entry.path().to_path_buf();
                if entry.kind() == EntryKind::Directory {
                    name.as_mut_os_string().push("/");
                }
                name
            })
            .collect();
        found.sort();
        for path in found {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    fn debian(native: bool) -> Vec<(OsString, TestFile)> {
        let format = if native {
            "3.0 (native)\n"
        } else {
            "3.0 (quilt)\n"
        };
        vec![
            (OsString::from("debian/control"), TestFile::from(CONTROL)),
            (
                OsString::from("debian/source/format"),
                TestFile::from(format),
            ),
            (
                OsString::from("debian/changelog"),
                TestFile::from(
                    "example (1.0-1) unstable; urgency=low\n\n  * x\n\n \
                     -- Jane <jane@example.com>  Mon, 01 Jan 2024 00:00:00 +0000\n",
                ),
            ),
        ]
    }

    fn orig(files: &[(&[u8], TestFile)]) -> Vec<(OsString, TestFile)> {
        files
            .iter()
            .map(|(path, file)| (OsString::from_vec(path.to_vec()), file.clone()))
            .collect()
    }

    #[test]
    fn test() {
        // A Source tree has no Orig files.
        let pass = vec![{
            let mut files = debian(false);
            files.extend(orig(&[(b"bokm\xe5l", TestFile::new("x\n"))]));
            files
        }];
        SourceTreeTester::new(
            UpstreamFileWithoutUtf8Name,
            pass,
            Vec::<Vec<(OsString, TestFile)>>::new(),
        )
        .test();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            SourcePackageCase::from(debian(false)),
            SourcePackageCase::from(debian(false))
                .orig(orig(&[(b"bokm\xc3\xa5l", TestFile::new("x\n"))])),
            // Only the Orig files count, not the Debian changes.
            SourcePackageCase::from({
                let mut files = debian(false);
                files.extend(orig(&[(b"bokm\xe5l", TestFile::new("x\n"))]));
                files
            }),
            // Native: no upstream.
            SourcePackageCase::from(debian(true)),
        ];
        let fail = vec![
            SourcePackageCase::from(debian(false))
                .orig(orig(&[(b"bokm\xe5l", TestFile::new("x\n"))])),
            SourcePackageCase::from(debian(false))
                .orig(orig(&[(b"bokm\\\xe5l", TestFile::new("x\n"))])),
            SourcePackageCase::from(debian(false)).orig(orig(&[
                (b"dir/bokm\xe5l", TestFile::new("x\n")),
                (b"x\xff\xfe", TestFile::symlink("target")),
                (b"bad-dir\xe5", TestFile::directory()),
            ])),
        ];
        SourcePackageTester::new(UpstreamFileWithoutUtf8Name, pass, fail).test_and_snapshot();
    }
}
