use std::collections::HashSet;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::text;

pub struct Md5sumsListsNonexistentFile;

declare_rule! {
    /// The md5sums control file names a path that is not a regular file or
    /// hardlink in the package.
    ///
    /// One leading `./` on the listed path is ignored. A symlink or directory
    /// at that path still counts as missing. Extra is the path. The Location
    /// is `md5sums` with no line.
    Md5sumsListsNonexistentFile,
    code = "LN0343",
    tag = "md5sums-lists-nonexistent-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for Md5sumsListsNonexistentFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(file) = text::load(ctx) else {
            return;
        };
        let Some(installed) = text::installed_files(ctx) else {
            return;
        };
        let found: HashSet<&str> = installed.iter().map(|item| item.path.as_str()).collect();
        for (path, _) in file.entries {
            if !found.contains(path.as_str()) {
                ctx.diagnostic(Diagnostic::error(path).with_location(text::at()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn listed(path: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new("hello"))])
            .control_file("md5sums", TestFile::new(body))
    }

    #[test]
    fn test() {
        let checksum = "5d41402abc4b2a76b9719d911017c592";
        let pass = vec![
            listed("usr/bin/foo", &format!("{checksum}  usr/bin/foo\n")),
            listed("usr/bin/foo", &format!("{checksum}  ./usr/bin/foo\n")),
            listed("usr/bin/foo", ""),
        ];
        let fail = vec![
            listed("usr/bin/foo", &format!("{checksum}  usr/bin/missing\n")),
            BinaryPackageCase::from(vec![(
                "usr/bin/foo".to_string(),
                TestFile::symlink("somewhere"),
            )])
            .control_file(
                "md5sums",
                TestFile::new(format!("{checksum}  usr/bin/foo\n")),
            ),
        ];
        BinaryPackageTester::new(Md5sumsListsNonexistentFile, pass, fail).test_and_snapshot();
    }
}
