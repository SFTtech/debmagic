use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::file_list::ChecksumAlgorithm;
use crate::lint::group_context::ChangesContext;
use crate::lint::rule::ChangesRule;

pub struct ChecksumCountMismatchInChangesFile;

declare_rule! {
    /// The `.changes` lists a different number of files with a checksum of
    /// some algorithm than it lists files in all. The files are the names of
    /// the `Files` and `Checksums-*` fields together; an algorithm is counted
    /// only when some file has a sum for it. Algorithms are reported in the
    /// order Md5, Sha1, Sha256. The extra is, for example, `3 Checksum-Sha256
    /// checksums != 4 files`.
    ChecksumCountMismatchInChangesFile,
    code = "LN1486",
    tag = "checksum-count-mismatch-in-changes-file",
    default_selected = true,
    experimental = false,
}

impl ChangesRule for ChecksumCountMismatchInChangesFile {
    fn run(&self, ctx: &mut ChangesContext<'_>) {
        let files = ctx.changes().files().files();
        let expected = files.len();
        let mut bad = Vec::new();
        for algorithm in [
            ChecksumAlgorithm::Md5,
            ChecksumAlgorithm::Sha1,
            ChecksumAlgorithm::Sha256,
        ] {
            let actual = files
                .iter()
                .filter(|file| file.checksum(algorithm).is_some())
                .count();
            if actual > 0 && actual != expected {
                bad.push(format!(
                    "{actual} Checksum-{} checksums != {expected} files",
                    algorithm.name()
                ));
            }
        }
        for extra in bad {
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

    fn changes(files: &[&str], sha256: &[&str]) -> ChangesCase {
        let mut text = String::from("Source: example\nVersion: 1.0-1\nFiles:\n");
        for name in files {
            text.push_str(&format!(" {MD5} 0 misc optional {name}\n"));
        }
        text.push_str("Checksums-Sha256:\n");
        for name in sha256 {
            text.push_str(&format!(" {SHA256} 0 {name}\n"));
        }
        ChangesCase::from(text)
    }

    #[test]
    fn changes_file() {
        let pass = vec![
            // No file at all, or every file with a sum for each algorithm.
            ChangesCase::from("Source: example\nVersion: 1.0-1\n"),
            changes(&["a.deb", "b.dsc"], &["a.deb", "b.dsc"]),
            // No Sha256 at all is not a count to compare.
            changes(&["a.deb", "b.dsc"], &[]),
            ChangesCase::from(format!(
                "Source: example\nChecksums-Sha1:\n {SHA1} 0 a.deb\n {SHA1} 0 b.deb\n"
            )),
        ];
        let fail = vec![
            changes(&["a.deb", "b.dsc"], &["a.deb"]),
            // A Md5 for one file of two names: the names are counted, not the Files lines.
            changes(&["a.deb"], &["a.deb", "b.dsc"]),
            ChangesCase::from(format!(
                "Source: example\nFiles:\n {MD5} 0 misc optional a.deb\n {MD5} 0 misc optional b.deb\nChecksums-Sha1:\n {SHA1} 0 a.deb\n"
            )),
        ];
        ChangesTester::new(ChecksumCountMismatchInChangesFile, pass, fail).test_and_snapshot();
    }
}
