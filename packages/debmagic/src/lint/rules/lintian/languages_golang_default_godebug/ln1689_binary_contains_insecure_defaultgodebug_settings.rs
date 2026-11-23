use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct BinaryContainsInsecureDefaultgodebugSettings;

declare_rule! {
    /// An ELF file in the Data files of the Binary package has a `DefaultGODEBUG=`
    /// setting in its strings, so a Go binary forces legacy or insecure
    /// behaviour through `DefaultGODEBUG` (for example `rsa1024min=0,tlssha1=1`).
    /// Reported at pedantic severity, at the file's path, with the text after
    /// `DefaultGODEBUG=` on that string line as the extra.
    ///
    /// Only the first `DefaultGODEBUG=` string of a file is reported, as
    /// lintian does. Strings are the printable runs of four or more characters
    /// (`strings --all`). Regular files and hardlinks whose File type is ELF
    /// are read, and `usr/lib/debug/` is skipped, as lintian has no strings
    /// for it.
    BinaryContainsInsecureDefaultgodebugSettings,
    code = "LN1689",
    tag = "binary-contains-insecure-defaultgodebug-settings",
    default_selected = false,
    experimental = false,
}

const NEEDLE: &str = "DefaultGODEBUG=";

impl BinaryPackageRule for BinaryContainsInsecureDefaultgodebugSettings {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(strings) = ctx.data_strings() else {
            return;
        };
        let mut findings: Vec<(PathBuf, String)> = Vec::new();
        for (path, text) in strings.iter() {
            let settings = text.lines().find_map(|line| {
                line.find(NEEDLE)
                    .map(|at| line[at + NEEDLE.len()..].to_string())
            });
            if let Some(settings) = settings {
                findings.push((path.to_path_buf(), settings));
            }
        }
        for (path, settings) in findings {
            ctx.diagnostic(
                Diagnostic::warn(settings)
                    .with_severity(Severity::Pedantic)
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
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    const BIN: &str = "usr/bin/example";

    fn elf_with(text: &[u8]) -> TestFile {
        let mut bytes = TestElf::executable().build();
        bytes.extend_from_slice(b"\0");
        bytes.extend_from_slice(text);
        bytes.extend_from_slice(b"\0");
        TestFile::from_bytes(bytes)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(vec![(BIN, elf_with(b"GODEBUG=rsa1024min=0"))]),
            BinaryPackageCase::from(vec![(BIN, elf_with(b"DefaultGODEBUG"))]),
            // Not an ELF file.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example.txt",
                TestFile::from("DefaultGODEBUG=tlssha1=1\n"),
            )]),
            // Debug symbols have no strings for lintian.
            BinaryPackageCase::from(vec![(
                "usr/lib/debug/usr/bin/example.debug",
                elf_with(b"DefaultGODEBUG=tlssha1=1"),
            )]),
            // A udeb without the setting passes.
            BinaryPackageCase::from(vec![(BIN, elf_with(b"plain text"))]).udeb(),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                BIN,
                elf_with(b"DefaultGODEBUG=rsa1024min=0,tlssha1=1"),
            )]),
            BinaryPackageCase::from(vec![(BIN, elf_with(b"DefaultGODEBUG=tlssha1=1"))]).udeb(),
        ];
        BinaryPackageTester::new(BinaryContainsInsecureDefaultgodebugSettings, pass, fail)
            .test_and_snapshot();
    }
}
