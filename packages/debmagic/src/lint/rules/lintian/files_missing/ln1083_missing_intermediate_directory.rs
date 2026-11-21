use std::collections::{BTreeSet, HashSet};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct MissingIntermediateDirectory;

declare_rule! {
    /// The Binary package's data contains files under a directory that the archive does not list
    /// as an entry of its own.
    ///
    /// Port of lintian's `missing-intermediate-directory` from the `files/missing` Check. Lintian
    /// adds a "faux" directory for every parent of an installed item that has no entry of its own;
    /// each such directory is reported with a trailing slash as the Location and an empty extra.
    /// A directory is not reported when the data tarball lists anything at that path, whatever its
    /// kind. Paths are compared as raw bytes, and each faux directory is reported once. Udebs are
    /// checked too. Only the Data files are inspected; a package without data files is clean.
    MissingIntermediateDirectory,
    code = "LN1083",
    tag = "missing-intermediate-directory",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MissingIntermediateDirectory {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let present: HashSet<&[u8]> = files
            .entries()
            .iter()
            .map(|entry| entry.path().as_os_str().as_bytes())
            .collect();
        let mut faux: BTreeSet<&[u8]> = BTreeSet::new();
        for entry in files.entries() {
            let bytes = entry.path().as_os_str().as_bytes();
            for (index, byte) in bytes.iter().enumerate() {
                if *byte != b'/' {
                    continue;
                }
                let parent = &bytes[..index];
                if !parent.is_empty() && !present.contains(parent) {
                    faux.insert(parent);
                }
            }
        }
        let locations: Vec<PathBuf> = faux
            .into_iter()
            .map(|parent| {
                let mut name = OsStr::from_bytes(parent).to_os_string();
                name.push("/");
                PathBuf::from(name)
            })
            .collect();
        for path in locations {
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

    /// Data files as `(path, entry)` pairs, the way a `.deb`'s data tarball has them.
    fn tree(files: &[(&str, TestFile)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            files
                .iter()
                .map(|(path, file)| (path.to_string(), file.clone()))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            tree(&[]),
            // Every parent is listed as a directory entry.
            tree(&[
                ("usr", TestFile::directory()),
                ("usr/share", TestFile::directory()),
                ("usr/share/doc", TestFile::directory()),
                ("usr/share/doc/example", TestFile::directory()),
                ("usr/share/doc/example/README", TestFile::new("text")),
            ]),
            // A top-level file has no parent directory to miss.
            tree(&[("README", TestFile::new("text"))]),
            // A symlink standing in for a parent path counts as present.
            tree(&[
                ("usr", TestFile::directory()),
                ("usr/lib64", TestFile::symlink("lib")),
                ("usr/lib64/x", TestFile::new("text")),
            ]),
        ];
        let fail = vec![
            // Lintian's own recipe: the directory exists, its parents do not.
            tree(&[(
                "usr/share/doc/files-missing-intermediate-dirs",
                TestFile::directory(),
            )]),
            tree(&[("usr/bin/tool", TestFile::new("text"))]),
            // A udeb is checked the same way.
            tree(&[("usr/bin/tool", TestFile::new("text"))]).udeb(),
            // Each faux directory is reported once, however many items sit below it.
            tree(&[
                ("usr/bin/a", TestFile::new("text")),
                ("usr/bin/b", TestFile::new("text")),
            ]),
        ];
        BinaryPackageTester::new(MissingIntermediateDirectory, pass, fail).test_and_snapshot();
    }
}
