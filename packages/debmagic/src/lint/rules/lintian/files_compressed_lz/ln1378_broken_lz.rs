use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::external_tool;
use crate::lint::rule::BinaryPackageRule;

pub struct BrokenLz;

declare_rule! {
    /// A file named `*.lz` is not a valid `lz` stream.
    ///
    /// Every installed regular file (or hardlink) whose name ends in
    /// `.lz`, in any case, is run through `lzip / clzip --test`. Reported when
    /// it exits non-zero. Extra is empty; the path is the Location. The Rule
    /// reports nothing when the tool is not installed. Applies to udebs too.
    BrokenLz,
    code = "LN1378",
    tag = "broken-lz",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BrokenLz {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(program) = external_tool::find("lzip").or_else(|| external_tool::find("clzip"))
        else {
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
                    .ends_with(".lz")
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
    const VALID: &[u8] = b"\x4c\x5a\x49\x50\x01\x0c\x00\x34\x19\x49\xee\x8d\xdd\x3d\x3a\xdf\xff\xff\xdd\x12\x00\x00\x20\x30\x3a\x36\x06\x00\x00\x00\x00\x00\x00\x00\x2a\x00\x00\x00\x00\x00\x00\x00";

    fn file(path: &str, bytes: &[u8]) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::from_bytes(bytes))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            file("usr/share/doc/pkg/ok.lz", VALID),
            // Not named as the format, so not checked.
            file("etc/plain.txt", b"not compressed\n"),
        ];
        let fail = vec![
            file("etc/not-an.lz", b"hello\n"),
            file("etc/NOT-AN.LZ", b"hello\n"),
        ];
        BinaryPackageTester::new(BrokenLz, pass, fail).test();
    }
}
