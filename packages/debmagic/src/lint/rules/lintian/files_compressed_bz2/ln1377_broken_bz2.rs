use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::external_tool;
use crate::lint::rule::BinaryPackageRule;

pub struct BrokenBz2;

declare_rule! {
    /// A file named `*.bz2` is not a valid `bz2` stream.
    ///
    /// Every installed regular file (or hardlink) whose name ends in
    /// `.bz2`, in any case, is run through `bzip2 --test`. Reported when
    /// it exits non-zero. Extra is empty; the path is the Location. The Rule
    /// reports nothing when the tool is not installed. Applies to udebs too.
    BrokenBz2,
    code = "LN1377",
    tag = "broken-bz2",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BrokenBz2 {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(program) = external_tool::find("bzip2") else {
            return;
        };
        let Some(files) = ctx.data_files() else {
            return;
        };
        let broken: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| entry.is_file())
            .filter(|entry| {
                entry
                    .path()
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .ends_with(".bz2")
            })
            .filter(|entry| {
                external_tool::rejects(files, entry, &program, &["--test"]) == Some(true)
            })
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in broken {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    /// A valid stream of `hello\n`, made by the tool itself.
    const VALID: &[u8] = b"\x42\x5a\x68\x39\x31\x41\x59\x26\x53\x59\xc1\xc0\x80\xe2\x00\x00\x01\x41\x00\x00\x10\x02\x44\xa0\x00\x30\xcd\x00\xc3\x46\x29\x97\x17\x72\x45\x38\x50\x90\xc1\xc0\x80\xe2";

    fn file(path: &str, bytes: &[u8]) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::from_bytes(bytes))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            file("usr/share/doc/pkg/ok.bz2", VALID),
            // Not named as the format, so not checked.
            file("etc/plain.txt", b"not compressed\n"),
        ];
        let fail = vec![
            file("etc/not-a.bz2", b"hello\n"),
            file("etc/NOT-AN.BZ2", b"hello\n"),
        ];
        BinaryPackageTester::new(BrokenBz2, pass, fail).test();
    }
}
