use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::text;

pub struct NoMd5sumsControlFile;

declare_rule! {
    /// A Binary package with installed files has no `md5sums` Control file, and
    /// one of its regular files is not a declared conffile.
    ///
    /// Only regular files count (hardlinks do not), and only the Installed
    /// files that are not listed in the `conffiles` Control file. A package
    /// with no installed entries at all is skipped. Reported as an info hint
    /// with no Extra and no Location.
    NoMd5sumsControlFile,
    code = "LN1419",
    tag = "no-md5sums-control-file",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for NoMd5sumsControlFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx
            .control_files()
            .is_some_and(|files| files.get("md5sums").is_some())
        {
            return;
        }
        let conffiles = text::conffiles(ctx);
        let Some(files) = ctx.data_files() else {
            return;
        };
        // Lintian skips a package whose installed list is empty, and a
        // regular file counts only when it is not a declared conffile.
        if files.entries().is_empty() {
            return;
        }
        let only_conffiles = files
            .entries()
            .iter()
            .filter(|entry| entry.kind() == EntryKind::RegularFile)
            .all(|entry| {
                entry
                    .path()
                    .to_str()
                    .is_some_and(|path| conffiles.contains(path))
            });
        if only_conffiles {
            return;
        }
        ctx.diagnostic(Diagnostic::warn("").with_severity(Severity::Info));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    #[test]
    fn binary() {
        let pass = vec![
            // An md5sums file is present.
            BinaryPackageCase::from(vec![("usr/bin/app", TestFile::new("x"))])
                .control_file("md5sums", TestFile::new("")),
            // Only declared conffiles are installed.
            BinaryPackageCase::from(vec![("etc/app.conf", TestFile::new("x"))])
                .control_file("conffiles", TestFile::new("/etc/app.conf\n")),
            // No installed entries at all.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // A hardlink is not counted; the regular file is a declared conffile.
            BinaryPackageCase::from(vec![
                ("usr/bin/app", TestFile::new("x")),
                (
                    "usr/bin/alias",
                    TestFile::new("").hardlink_to("usr/bin/app"),
                ),
            ])
            .control_file("conffiles", TestFile::new("/usr/bin/app\n")),
        ];
        let fail = vec![
            // A regular file that is not a conffile.
            BinaryPackageCase::from(vec![("usr/bin/app", TestFile::new("x"))]),
            // One conffile and one other regular file.
            BinaryPackageCase::from(vec![
                ("etc/app.conf", TestFile::new("x")),
                ("usr/bin/app", TestFile::new("x")),
            ])
            .control_file("conffiles", TestFile::new("/etc/app.conf\n")),
        ];
        BinaryPackageTester::new(NoMd5sumsControlFile, pass, fail).test_and_snapshot();
    }
}
