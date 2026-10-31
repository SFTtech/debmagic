use std::collections::HashMap;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::text;

pub struct Md5sumMismatch;

declare_rule! {
    /// The md5 checksum in the md5sums control file differs from the md5 of
    /// the installed file's bytes.
    ///
    /// The comparison is case-sensitive. Empty file bytes still have a
    /// checksum. Extra is the path. The Location is `md5sums` with no line.
    Md5sumMismatch,
    code = "LN0345",
    tag = "md5sum-mismatch",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for Md5sumMismatch {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(file) = text::load(ctx) else {
            return;
        };
        let Some(installed) = text::installed_files(ctx) else {
            return;
        };
        let by_path: HashMap<&str, &[u8]> = installed
            .iter()
            .filter_map(|item| Some((item.path.as_str(), item.bytes.as_deref()?)))
            .collect();
        for (path, checksum) in file.entries {
            let Some(bytes) = by_path.get(path.as_str()) else {
                continue;
            };
            if text::md5_hex(bytes) != checksum {
                ctx.diagnostic(Diagnostic::error(path).with_location(text::at()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn packaged(path: &str, body: &str, md5sums: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path.to_string(), TestFile::new(body))])
            .control_file("md5sums", TestFile::new(md5sums))
    }

    #[test]
    fn test() {
        let hello = "5d41402abc4b2a76b9719d911017c592";
        let empty = "d41d8cd98f00b204e9800998ecf8427e";
        let pass = vec![
            packaged("usr/bin/foo", "hello", &format!("{hello}  usr/bin/foo\n")),
            packaged("usr/bin/foo", "hello", &format!("{hello}  ./usr/bin/foo\n")),
            packaged("usr/bin/empty", "", &format!("{empty}  usr/bin/empty\n")),
            packaged("usr/bin/foo", "hello", ""),
        ];
        let fail = vec![
            packaged(
                "usr/bin/foo",
                "hello",
                "00000000000000000000000000000000  usr/bin/foo\n",
            ),
            packaged(
                "usr/bin/foo",
                "hello",
                "5D41402ABC4B2A76B9719D911017C592  usr/bin/foo\n",
            )
            .udeb(),
        ];
        BinaryPackageTester::new(Md5sumMismatch, pass, fail).test_and_snapshot();
    }
}
