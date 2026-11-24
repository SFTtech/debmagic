use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::digest::sibling_digest;
use crate::lint::file_list::ChecksumAlgorithm;
use crate::lint::group_context::ChangesContext;
use crate::lint::rule::ChangesRule;

pub struct ChecksumMismatchInChangesFile;

declare_rule! {
    /// Reports a checksum line of the `.changes` (in the `Files` or a
    /// `Checksums-*` field) whose sum differs from the digest of the file of
    /// the same name next to the `.changes` on disk. The extra is the checksum
    /// line name and the file name (`Checksum-Sha256 __some-file__`).
    ///
    /// Lintian's Check reports the files in hash order and checks the
    /// algorithms Md5, Sha1, Sha256 in that order for each file; this Rule
    /// reports them in the order of the fields. Differences from lintian: a
    /// sibling that cannot be read (missing, or no permission) is not
    /// compared, where lintian compares against an undefined checksum; the
    /// sum is compared as written, so an upper-case hex sum mismatches, as in
    /// lintian.
    ChecksumMismatchInChangesFile,
    code = "LN1484",
    tag = "checksum-mismatch-in-changes-file",
    default_selected = true,
    experimental = false,
}

impl ChangesRule for ChecksumMismatchInChangesFile {
    fn run(&self, ctx: &mut ChangesContext<'_>) {
        let mut mismatches = Vec::new();
        for file in ctx.changes().files().files() {
            let name = file.name();
            let path = ctx.sibling(name);
            for algorithm in [
                ChecksumAlgorithm::Md5,
                ChecksumAlgorithm::Sha1,
                ChecksumAlgorithm::Sha256,
            ] {
                let Some(checksum) = file.checksum(algorithm) else {
                    continue;
                };
                let Ok(actual) = sibling_digest(&path, algorithm.name()) else {
                    continue;
                };
                if checksum.sum() != actual {
                    mismatches.push(format!("Checksum-{} {name}", algorithm.name()));
                }
            }
        }
        for extra in mismatches {
            ctx.diagnostic(Diagnostic::error(extra));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{ChangesCase, ChangesTester};

    use super::*;

    const CONTENT: &str = "twelve bytes"; // 12 bytes
    const MD5: &str = "c91fa94e14cf2fa5474beceff788c0dc";
    const SHA1: &str = "b7e96747b2c1a8d418e7bf3167b33d379d1af9fa";
    const SHA256: &str = "d4ce2c527afe674c7a086bd74e256019e3d5dcdb31eeb6eaadef5ada8c4383b9";
    const WRONG: &str = "0000000000000000000000000000000000000000";

    fn changes(fields: &str) -> ChangesCase {
        ChangesCase::from(format!("Source: example\nVersion: 1.0\n{fields}"))
    }

    #[test]
    fn changes_file() {
        let pass = vec![
            changes(&format!("Files:\n {MD5} 12 devel optional a_1.0.deb\n"))
                .file("a_1.0.deb", CONTENT),
            changes(&format!(
                "Files:\n {MD5} 12 devel optional a_1.0.deb\nChecksums-Sha1:\n {SHA1} 12 a_1.0.deb\nChecksums-Sha256:\n {SHA256} 12 a_1.0.deb\n"
            ))
            .file("a_1.0.deb", CONTENT),
            // The file is not next to the .changes: nothing to compare with.
            changes(&format!("Files:\n {WRONG} 12 devel optional a_1.0.deb\n")),
        ];
        let fail = vec![
            changes(&format!(
                "Files:\n {WRONG} 12 devel optional __some-file__\nChecksums-Sha1:\n {WRONG} 12 __some-file__\nChecksums-Sha256:\n {WRONG} 12 __some-file__\n"
            ))
            .file("__some-file__", CONTENT),
            changes(&format!(
                "Files:\n {MD5} 12 devel optional a_1.0.deb\nChecksums-Sha1:\n {WRONG} 12 a_1.0.deb\n"
            ))
            .file("a_1.0.deb", CONTENT),
            // Upper-case hex is not the sum the file has.
            changes(&format!(
                "Files:\n {} 12 devel optional a_1.0.deb\n",
                MD5.to_ascii_uppercase()
            ))
            .file("a_1.0.deb", CONTENT),
        ];
        ChangesTester::new(ChecksumMismatchInChangesFile, pass, fail).test_and_snapshot();
    }
}
