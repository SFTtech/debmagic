use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct PackageModifiesLdSoSearchPath;

declare_rule! {
    /// The Binary package installs something under `etc/ld.so.conf.d/` without being `libc*`.
    ///
    /// Port of lintian's `package-modifies-ld.so-search-path` from the `files/ld-so` Check. Any
    /// installed item below `etc/ld.so.conf.d/` is reported, directories included (with a trailing
    /// slash), and the extra is empty. Packages whose name starts with `libc` are exempt, as in
    /// lintian. Udebs are checked too.
    PackageModifiesLdSoSearchPath,
    code = "LN1082",
    tag = "package-modifies-ld.so-search-path",
    default_selected = true,
    experimental = false,
}

const LD_SO_CONF_DIR: &[u8] = b"etc/ld.so.conf.d/";

impl BinaryPackageRule for PackageModifiesLdSoSearchPath {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.package_name().starts_with("libc") {
            return;
        }
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<PathBuf> = files
            .entries()
            .iter()
            .filter(|entry| {
                let bytes = entry.path().as_os_str().as_encoded_bytes();
                bytes.len() > LD_SO_CONF_DIR.len() && bytes.starts_with(LD_SO_CONF_DIR)
            })
            .map(|entry| {
                if entry.kind() == EntryKind::Directory {
                    PathBuf::from(format!("{}/", entry.path().display()))
                } else {
                    entry.path().to_path_buf()
                }
            })
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
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
            tree(&[("etc/ld.so.conf", TestFile::new("include"))]),
            tree(&[("etc/ld.so.conf.d", TestFile::directory())]),
            tree(&[("etc/ld.so.conf.d/", TestFile::directory())]),
            tree(&[("usr/lib/ld.so.conf.d/x.conf", TestFile::new("/usr/lib"))]),
            tree(&[(
                "etc/ld.so.conf.d/lintian-lib.conf",
                TestFile::new("/usr/lib"),
            )])
            .package("libc6"),
            tree(&[("etc/ld.so.conf.d/x.conf", TestFile::new("/opt/lib"))]).package("libcfoo"),
        ];
        let fail = vec![
            tree(&[(
                "etc/ld.so.conf.d/lintian-lib.conf",
                TestFile::new("/opt/lib"),
            )]),
            tree(&[("etc/ld.so.conf.d/x.conf", TestFile::new("/opt/lib"))]).udeb(),
            tree(&[("etc/ld.so.conf.d/sub", TestFile::directory())]),
            // A symlink below the directory is reported too; lintian checks every item.
            tree(&[("etc/ld.so.conf.d/link", TestFile::symlink("/opt/lib"))]),
            tree(&[("etc/ld.so.conf.d/a.conf", TestFile::new("/a"))]).package("lib-x"),
        ];
        BinaryPackageTester::new(PackageModifiesLdSoSearchPath, pass, fail).test_and_snapshot();
    }
}
