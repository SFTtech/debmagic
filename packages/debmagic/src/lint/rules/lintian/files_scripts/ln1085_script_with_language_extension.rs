use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

pub struct ScriptWithLanguageExtension;

declare_rule! {
    /// A Binary package installs a script into a directory on the system `PATH`
    /// (`bin/`, `sbin/`, `games/`, or their `usr/` counterparts) whose file name
    /// ends in a scripting-language extension: `.pl`, `.py`, `.php`, `.rb`,
    /// `.sh`, `.bsh`, `.csh` or `.tcl`. Policy 10.4 asks for such names to be
    /// extension-free, so the implementation language can change without
    /// renaming the command.
    ///
    /// Walks Data files that are regular files or hardlinks. Symlinks,
    /// directories and other non-file entries are not hits, and nested paths
    /// (`usr/bin/foo/bar.sh`) are not hits. The Location is the script path;
    /// the extra is empty.
    ScriptWithLanguageExtension,
    code = "LN1085",
    tag = "script-with-language-extension",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ScriptWithLanguageExtension {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let pattern =
            Regex::new(r"(?-u)\A(?:usr/)?(?:s?bin|games)/[^/]+\.(?:p[ly]|php|rb|[bc]?sh|tcl)\z")
                .expect("valid regex");
        let paths: Vec<PathBuf> = files
            .query()
            .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
            .into_iter()
            .filter(|entry| pattern.is_match(entry.path().as_os_str().as_bytes()))
            .map(|entry| entry.path().to_path_buf())
            .collect();
        for path in paths {
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

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![("usr/bin/test", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("usr/share/test.sh", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("usr/lib/foo/test.sh", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("usr/bin/test.exe", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("usr/bin/test.bash", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![(
                "usr/bin/test.sh",
                TestFile::symlink("/usr/share/test"),
            )]),
            BinaryPackageCase::from(vec![("usr/bin/test.sh", TestFile::directory())]),
            BinaryPackageCase::from(vec![("usr/bin/foo/test.sh", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("usr/bin/.sh", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("usr/bin/test.sh", TestFile::fifo())]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("usr/bin/test.sh", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("bin/test.sh", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("usr/sbin/test.pl", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("usr/games/test.py", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("sbin/test.php", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("usr/bin/test.rb", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("usr/bin/test.csh", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("usr/bin/test.tcl", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("usr/bin/test.sh", TestFile::new("x").hardlink())]),
            // A udeb is still a Binary package.
            BinaryPackageCase::from(vec![("usr/bin/test.sh", TestFile::new("x"))]).udeb(),
        ];
        BinaryPackageTester::new(ScriptWithLanguageExtension, pass, fail).test_and_snapshot();
    }
}
