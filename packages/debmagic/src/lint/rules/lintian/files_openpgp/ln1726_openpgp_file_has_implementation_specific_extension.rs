use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::Entry;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct OpenpgpFileHasImplementationSpecificExtension;

declare_rule! {
    /// An OpenPGP file is named with the `.gpg` extension.
    ///
    /// Walks Data files. A file or hardlink whose path ends in `.gpg` is
    /// reported: `.pgp` is the implementation-neutral extension. Symlinks,
    /// directories, and other kinds are not reported. The check is on the
    /// whole path, so the name need not be a key file. Udebs are checked too.
    /// Extra is empty; the Location is the file.
    OpenpgpFileHasImplementationSpecificExtension,
    code = "LN1726",
    tag = "openpgp-file-has-implementation-specific-extension",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for OpenpgpFileHasImplementationSpecificExtension {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| is_gpg_file(entry))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in hits {
            ctx.diagnostic(
                Diagnostic::warn("")
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

fn is_gpg_file(entry: &Entry) -> bool {
    entry.is_file()
        && entry
            .path()
            .to_str()
            .is_some_and(|path| path.ends_with(".gpg"))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(entries: Vec<(String, TestFile)>) -> BinaryPackageCase {
        BinaryPackageCase::from(entries)
    }

    #[test]
    fn binary() {
        let pass = vec![
            case(vec![]),
            // The neutral extension.
            case(vec![(
                "usr/share/keyrings/archive-keyring.pgp".to_string(),
                TestFile::new("x"),
            )]),
            // The extension must be the end of the name.
            case(vec![(
                "usr/share/keyrings/archive-keyring.gpg.txt".to_string(),
                TestFile::new("x"),
            )]),
            case(vec![("usr/share/foo/gpg".to_string(), TestFile::new("x"))]),
            // A symlink is not a file.
            case(vec![(
                "usr/share/keyrings/archive-keyring.gpg".to_string(),
                TestFile::symlink("archive-keyring.pgp"),
            )]),
            // A directory is not a file.
            case(vec![(
                "usr/share/keyrings/dir.gpg".to_string(),
                TestFile::directory(),
            )]),
        ];
        let fail = vec![
            case(vec![(
                "usr/share/keyrings/archive-keyring.gpg".to_string(),
                TestFile::new("x"),
            )]),
            // A hardlink is a file for lintian.
            case(vec![
                ("usr/share/foo/a.pgp".to_string(), TestFile::new("x")),
                (
                    "usr/share/foo/a.gpg".to_string(),
                    TestFile::new("x").hardlink_to("usr/share/foo/a.pgp"),
                ),
            ]),
            // Any directory, not only keyrings.
            case(vec![(
                "etc/apt/trusted.gpg".to_string(),
                TestFile::new("x"),
            )]),
            // Udebs are checked too.
            case(vec![(
                "usr/share/keyrings/k.gpg".to_string(),
                TestFile::new("x"),
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(OpenpgpFileHasImplementationSpecificExtension, pass, fail)
            .test_and_snapshot();
    }
}
