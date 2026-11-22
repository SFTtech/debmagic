use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_list::ChecksumAlgorithm;
use crate::lint::group_context::ChangesContext;
use crate::lint::rule::ChangesRule;

pub struct FileSizeMismatchInChangesFile;

declare_rule! {
    /// Reports a file of the `.changes` whose size in the `Files` line, or in
    /// a `Checksums-*` line, differs from the size of the file next to the
    /// `.changes` on disk, or from the size of another checksum line of the
    /// same file. The extra is the file name, the size the field states, `!=`
    /// and the size found (`__some-file__ 0 != 12`).
    ///
    /// Lintian's Check reports the files in hash order and checks the sizes
    /// Md5, Sha1, Sha256 in that order for each file; this Rule reports them
    /// in the order of the fields. Differences from lintian: a sibling that
    /// cannot be stat'ed (missing, or no permission) is not compared, where
    /// lintian compares against an undefined size; a checksum line without a
    /// number as its size is not compared; a file named only in a
    /// `Checksums-*` field has no `Files` size to compare, so only its checksum
    /// line sizes are compared with the sibling.
    FileSizeMismatchInChangesFile,
    code = "LN1490",
    tag = "file-size-mismatch-in-changes-file",
    default_selected = true,
    experimental = false,
}

impl ChangesRule for FileSizeMismatchInChangesFile {
    fn run(&self, ctx: &mut ChangesContext<'_>) {
        let mut mismatches = Vec::new();
        for file in ctx.changes().files().files() {
            let name = file.name();
            let actual = std::fs::metadata(ctx.sibling(name))
                .ok()
                .map(|metadata| metadata.len());
            let stated = file.size();
            if let (Some(stated), Some(actual)) = (stated, actual)
                && stated != actual
            {
                mismatches.push(format!("{name} {stated} != {actual}"));
            }
            let Some(stated) = stated else { continue };
            for algorithm in [
                ChecksumAlgorithm::Md5,
                ChecksumAlgorithm::Sha1,
                ChecksumAlgorithm::Sha256,
            ] {
                let Some(checksum) = file.checksum(algorithm) else {
                    continue;
                };
                let Some(filesize) = checksum.size() else {
                    continue;
                };
                if stated != filesize {
                    mismatches.push(format!("{name} {stated} != {filesize}"));
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

    const MD5: &str = "d41d8cd98f00b204e9800998ecf8427e";
    const SHA1: &str = "da39a3ee5e6b4b0d3255bfef95601890afd80709";
    const SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const CONTENT: &str = "twelve bytes"; // 12 bytes

    fn changes(fields: &str) -> ChangesCase {
        ChangesCase::from(format!("Source: example\nVersion: 1.0\n{fields}"))
    }

    #[test]
    fn changes_file() {
        let pass = vec![
            changes(&format!("Files:\n {MD5} 12 devel optional a_1.0.deb\n")).file("a_1.0.deb", CONTENT),
            changes(&format!(
                "Files:\n {MD5} 12 devel optional a_1.0.deb\nChecksums-Sha1:\n {SHA1} 12 a_1.0.deb\nChecksums-Sha256:\n {SHA256} 12 a_1.0.deb\n"
            ))
            .file("a_1.0.deb", CONTENT),
            // The file is not next to the .changes: nothing to compare with.
            changes(&format!("Files:\n {MD5} 12 devel optional a_1.0.deb\n")),
            // Only a Checksums field names it: no Files size, and no file.
            changes(&format!("Checksums-Sha1:\n {SHA1} 0 b_1.0.deb\n")),
        ];
        let fail = vec![
            changes(&format!("Files:\n {MD5} 0 devel optional __some-file__\nChecksums-Sha1:\n {SHA1} 0 __some-file__\nChecksums-Sha256:\n {SHA256} 0 __some-file__\n"))
                .file("__some-file__", CONTENT),
            changes(&format!(
                "Files:\n {MD5} 12 devel optional a_1.0.deb\nChecksums-Sha1:\n {SHA1} 0 a_1.0.deb\n"
            ))
            .file("a_1.0.deb", CONTENT),
            changes(&format!(
                "Files:\n {MD5} 12 devel optional a_1.0.deb\nChecksums-Sha1:\n {SHA1} 0 a_1.0.deb\nChecksums-Sha256:\n {SHA256} 0 a_1.0.deb\n"
            ))
            .file("a_1.0.deb", CONTENT),
            changes(&format!(
                "Files:\n {MD5} 12 devel optional a_1.0.deb\n {MD5} 3 devel optional b_1.0.deb\n"
            ))
            .file("a_1.0.deb", CONTENT)
            .file("b_1.0.deb", CONTENT),
        ];
        ChangesTester::new(FileSizeMismatchInChangesFile, pass, fail).test_and_snapshot();
    }
}
