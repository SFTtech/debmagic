use std::collections::HashSet;
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;
use crate::lint::rules::lintian::debian_changelog::source::package_is_native;
use crate::lint::run::pointer_path;

pub struct PatchedFileWithoutUtf8Name;

declare_rule! {
    /// A non-native Source package adds a file whose name is not UTF-8.
    ///
    /// Only for non-native packages (lintian's `!native`). Each Patched files
    /// entry whose path is not valid UTF-8, unless it is also an Orig file or
    /// lies under the quilt `.pc/` directory. Directories are checked too,
    /// with the trailing slash lintian gives them. Extra is the name with every
    /// byte read as Latin-1, as lintian prints it. No Location. Native packages
    /// get `native-source-file-without-utf8-name` instead. A Source tree has no
    /// Orig files, so every such path counts there.
    PatchedFileWithoutUtf8Name,
    code = "LN1247",
    tag = "patched-file-without-utf8-name",
    default_selected = true,
    experimental = false,
}

impl SourceRule for PatchedFileWithoutUtf8Name {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if package_is_native(ctx) {
            return;
        }
        let orig: HashSet<PathBuf> = ctx
            .orig_files()
            .map(|tree| {
                tree.entries()
                    .iter()
                    .map(|entry| entry.path().to_path_buf())
                    .collect()
            })
            .unwrap_or_default();
        let Some(files) = ctx.files() else {
            return;
        };
        let mut found: Vec<PathBuf> = Vec::new();
        for entry in files.entries() {
            let path = entry.path();
            if path.to_str().is_some() || orig.contains(path) {
                continue;
            }
            // Perl's `^.pc/` on the name: the first byte is any one character.
            let bytes = path.as_os_str().as_encoded_bytes();
            if bytes.len() > 3 && bytes[1..].starts_with(b"pc/") {
                continue;
            }
            let mut name = path.to_path_buf();
            if entry.kind() == EntryKind::Directory {
                name.as_mut_os_string().push("/");
            }
            found.push(name);
        }
        found.sort();
        for name in found {
            ctx.diagnostic(Diagnostic::error(pointer_path(&name)));
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

    fn tree(native: bool, extra: &[(&[u8], TestFile)]) -> Vec<(OsString, TestFile)> {
        let format = if native {
            "3.0 (native)\n"
        } else {
            "3.0 (quilt)\n"
        };
        let mut files = vec![
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
        ];
        files.extend(
            extra
                .iter()
                .map(|(path, file)| (OsString::from_vec(path.to_vec()), file.clone())),
        );
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(false, &[]),
            tree(false, &[(b"bokm\xc3\xa5l", TestFile::new("x\n"))]),
            tree(false, &[(b"dir/bokm\xc3\xa5l", TestFile::new("x\n"))]),
            tree(false, &[(b".pc/bokm\xe5l", TestFile::new("x\n"))]),
            // Native: native-source-file-without-utf8-name.
            tree(true, &[(b"bokm\xe5l", TestFile::new("x\n"))]),
        ];
        let fail = vec![
            tree(false, &[(b"bokm\xe5l", TestFile::new("x\n"))]),
            tree(false, &[(b"bokm\\\xe5l", TestFile::new("x\n"))]),
            tree(false, &[(b"dir/bokm\xe5l", TestFile::new("x\n"))]),
            tree(false, &[(b"bad-dir\xe5", TestFile::directory())]),
            tree(false, &[(b"x\xff\xfe", TestFile::symlink("target"))]),
        ];
        SourceTreeTester::new(PatchedFileWithoutUtf8Name, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            SourcePackageCase::from(tree(true, &[(b"bokm\xe5l", TestFile::new("x\n"))])),
            SourcePackageCase::from(tree(false, &[])).orig(vec![(
                OsString::from_vec(b"bokm\xe5l".to_vec()),
                TestFile::new("x\n"),
            )]),
        ];
        let fail = vec![SourcePackageCase::from(tree(
            false,
            &[(b"bokm\xe5l", TestFile::new("x\n"))],
        ))];
        SourcePackageTester::new(PatchedFileWithoutUtf8Name, pass, fail).test_and_snapshot();
    }
}
