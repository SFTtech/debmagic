use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::external_tool;
use crate::lint::rule::BinaryPackageRule;

pub struct BrokenLzo;

declare_rule! {
    /// A file named `*.lzo` is not a valid `lzo` stream.
    ///
    /// Every installed regular file (or hardlink) whose name ends in
    /// `.lzo`, in any case, is run through `lzop --test`. Reported when
    /// it exits non-zero. Extra is empty; the path is the Location. The Rule
    /// reports nothing when the tool is not installed. Applies to udebs too.
    BrokenLzo,
    code = "LN1379",
    tag = "broken-lzo",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BrokenLzo {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(program) = external_tool::find("lzop") else {
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
                    .ends_with(".lzo")
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
    const VALID: &[u8] = b"\x89\x4c\x5a\x4f\x00\x0d\x0a\x1a\x0a\x10\x40\x20\xa0\x09\x40\x01\x05\x03\x00\x00\x09\x00\x00\x81\xb4\x6b\x03\xe4\xd6\x00\x00\x00\x00\x09\x70\x6c\x61\x69\x6e\x2e\x74\x78\x74\x78\x48\x08\x74\x00\x00\x00\x06\x00\x00\x00\x06\x08\x4b\x02\x1f\x68\x65\x6c\x6c\x6f\x0a\x00\x00\x00\x00";

    fn file(path: &str, bytes: &[u8]) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::from_bytes(bytes))])
    }

    #[test]
    fn binary() {
        // The Rule reports nothing without the tool, so the fail cases need it.
        if external_tool::find("lzop").is_none() {
            eprintln!("skipped: lzop is not installed");
            return;
        }
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            file("usr/share/doc/pkg/ok.lzo", VALID),
            // Not named as the format, so not checked.
            file("etc/plain.txt", b"not compressed\n"),
        ];
        let fail = vec![
            file("etc/not-an.lzo", b"hello\n"),
            file("etc/NOT-AN.LZO", b"hello\n"),
        ];
        BinaryPackageTester::new(BrokenLzo, pass, fail).test();
    }
}
