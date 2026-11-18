use std::collections::HashSet;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::text;

pub struct FileMissingInMd5sums;

declare_rule! {
    /// A regular file or hardlink is not listed in md5sums.
    ///
    /// Paths listed in the conffiles control file are skipped, as are paths
    /// under `var/lib/aspell/` and `var/lib/ispell/`. Symlinks and directories
    /// are not expected in md5sums. Extra is the path. The Location is
    /// `md5sums` with no line.
    FileMissingInMd5sums,
    code = "LN0344",
    tag = "file-missing-in-md5sums",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for FileMissingInMd5sums {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(file) = text::load(ctx) else {
            return;
        };
        let Some(installed) = text::installed_files(ctx) else {
            return;
        };
        let declared = text::conffiles(ctx);
        let listed: HashSet<&str> = file.entries.iter().map(|(path, _)| path.as_str()).collect();
        for item in installed {
            if listed.contains(item.path.as_str())
                || declared.contains(&item.path)
                || text::aspell_path(&item.path)
            {
                continue;
            }
            ctx.diagnostic(Diagnostic::warn(item.path).with_location(text::at()));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn with_files(files: Vec<(&str, TestFile)>, md5sums: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(files).control_file("md5sums", TestFile::new(md5sums))
    }

    #[test]
    fn test() {
        let checksum = "5d41402abc4b2a76b9719d911017c592";
        let pass = vec![
            with_files(
                vec![("usr/bin/foo", TestFile::new("hello"))],
                &format!("{checksum}  usr/bin/foo\n"),
            ),
            with_files(
                vec![("usr/bin/foo", TestFile::new("hello"))],
                &format!("{checksum}  ./usr/bin/foo\n"),
            ),
            with_files(
                vec![("etc/foo.conf", TestFile::new("hello"))],
                &format!("{checksum}  usr/bin/other\n"),
            )
            .control_file(
                "conffiles",
                TestFile::new("remove-on-upgrade /etc/foo.conf\n"),
            ),
            with_files(
                vec![("var/lib/aspell/en.rws", TestFile::new("hello"))],
                &format!("{checksum}  usr/share/kept\n"),
            ),
            with_files(
                vec![("var/lib/ispell/en.hash", TestFile::new("hello"))],
                &format!("{checksum}  usr/share/kept\n"),
            ),
            with_files(
                vec![("usr/bin/link", TestFile::symlink("foo"))],
                &format!("{checksum}  usr/bin/somewhere\n"),
            ),
            with_files(vec![("usr/bin/foo", TestFile::new("hello"))], ""),
        ];
        let fail = vec![
            with_files(
                vec![("usr/share/doc/changelog", TestFile::new("hello"))],
                &format!("{checksum}  usr/bin/other\n"),
            ),
            with_files(
                vec![("etc/improper-link", TestFile::new("hello").hardlink())],
                &format!("{checksum}  usr/bin/other\n"),
            ),
            with_files(
                vec![("var/lib/spell/foo", TestFile::new("hello"))],
                &format!("{checksum}  usr/bin/other\n"),
            ),
        ];
        BinaryPackageTester::new(FileMissingInMd5sums, pass, fail).test_and_snapshot();
    }
}
