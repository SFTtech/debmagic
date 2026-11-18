use std::collections::HashSet;
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;
use crate::lint::rules::lintian::debian_changelog::source::package_is_native;
use crate::lint::run::pointer_path;

pub struct NativeSourceFileWithoutUtf8Name;

declare_rule! {
    /// A native Source package has a file whose name is not UTF-8.
    ///
    /// Only for native packages (lintian's `native`). Each Patched files entry
    /// whose path is not valid UTF-8, unless it is also an Orig file or lies
    /// under the quilt `.pc/` directory. Directories are checked too, with the
    /// trailing slash lintian gives them. Extra is the name with every byte
    /// read as Latin-1, as lintian prints it. No Location. Non-native packages
    /// get `patched-file-without-utf8-name` instead.
    NativeSourceFileWithoutUtf8Name,
    code = "LN0843",
    tag = "native-source-file-without-utf8-name",
    default_selected = true,
    experimental = false,
}

impl SourceRule for NativeSourceFileWithoutUtf8Name {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if !package_is_native(ctx) {
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
            tree(true, &[]),
            tree(true, &[(b"bokm\xc3\xa5l", TestFile::new("x\n"))]),
            tree(true, &[(b"dir/bokm\xc3\xa5l", TestFile::new("x\n"))]),
            tree(true, &[(b".pc/bokm\xe5l", TestFile::new("x\n"))]),
            // Not native: patched-file-without-utf8-name.
            tree(false, &[(b"bokm\xe5l", TestFile::new("x\n"))]),
        ];
        let fail = vec![
            tree(true, &[(b"bokm\xe5l", TestFile::new("x\n"))]),
            tree(true, &[(b"bokm\\\xe5l", TestFile::new("x\n"))]),
            tree(true, &[(b"dir/bokm\xe5l", TestFile::new("x\n"))]),
            tree(true, &[(b"bad-dir\xe5", TestFile::directory())]),
            tree(true, &[(b"x\xff\xfe", TestFile::symlink("target"))]),
        ];
        SourceTreeTester::new(NativeSourceFileWithoutUtf8Name, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![
            SourcePackageCase::from(tree(false, &[(b"bokm\xe5l", TestFile::new("x\n"))])),
            SourcePackageCase::from(tree(true, &[])).orig(vec![(
                OsString::from_vec(b"orig\xe5".to_vec()),
                TestFile::new("x\n"),
            )]),
        ];
        let fail = vec![SourcePackageCase::from(tree(
            true,
            &[(b"bokm\xe5l", TestFile::new("x\n"))],
        ))];
        SourcePackageTester::new(NativeSourceFileWithoutUtf8Name, pass, fail).test_and_snapshot();
    }
}
