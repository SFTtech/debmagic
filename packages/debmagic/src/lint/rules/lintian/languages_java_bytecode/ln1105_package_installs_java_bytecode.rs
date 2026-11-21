use std::os::unix::ffi::OsStrExt;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct PackageInstallsJavaBytecode;

declare_rule! {
    /// A compiled Java class file (`*.class` starting with the `CAFEBABE`
    /// magic) is installed outside test, example, demo, sample and doc paths.
    ///
    /// Walks the Data files, following lintian's `visit_installed_files`:
    /// only regular files and hardlinks count, so a symlink named `*.class`
    /// is not read. A path is skipped when its bytes contain `WEB-INF`,
    /// `demo`, `doc`, `example`, `sample` or `test`. Extra is empty; Location
    /// is the path, with no line number.
    PackageInstallsJavaBytecode,
    code = "LN1105",
    tag = "package-installs-java-bytecode",
    default_selected = true,
    experimental = false,
}

/// Leading bytes of a compiled Java class file.
const CLASS_MAGIC: &[u8] = b"\xCA\xFE\xBA\xBE";

/// Path fragments lintian exempts from the check.
const EXEMPT_FRAGMENTS: &[&[u8]] = &[b"WEB-INF", b"demo", b"doc", b"example", b"sample", b"test"];

impl BinaryPackageRule for PackageInstallsJavaBytecode {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for entry in files.entries().iter().filter(|entry| entry.is_file()) {
            let path = entry.path();
            let name = path.as_os_str().as_bytes();
            if !name.ends_with(b".class")
                || EXEMPT_FRAGMENTS
                    .iter()
                    .any(|fragment| contains(name, fragment))
            {
                continue;
            }
            let Ok(content) = files.read(entry) else {
                continue;
            };
            if content.starts_with(CLASS_MAGIC) {
                hits.push(path.to_path_buf());
            }
        }
        for path in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    const CLASS: &[u8] = b"\xCA\xFE\xBA\xBE\x00\x00\x00\x41rest";

    fn data(path: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, file)])
    }

    #[test]
    fn test() {
        let pass = vec![
            data("usr/share/java/foo/mini.txt", TestFile::from_bytes(CLASS)),
            // Bad magic.
            data(
                "usr/share/java/foo/Mini.class",
                TestFile::new("class file, not really\n"),
            ),
            // Too short for the magic.
            data(
                "usr/share/java/foo/Short.class",
                TestFile::from_bytes(&CLASS[..3]),
            ),
            // Exempt paths.
            data("usr/share/doc/foo/Demo.class", TestFile::from_bytes(CLASS)),
            data(
                "usr/share/java/foo/test/Mini.class",
                TestFile::from_bytes(CLASS),
            ),
            data(
                "usr/lib/foo/WEB-INF/classes/A.class",
                TestFile::from_bytes(CLASS),
            ),
            // A symlink is not read.
            data(
                "usr/share/java/foo/Link.class",
                TestFile::symlink("Mini.class"),
            ),
            data("usr/bin/empty", TestFile::new("")),
        ];
        let fail = vec![
            data("usr/share/java/foo/mini.class", TestFile::from_bytes(CLASS)),
            // A hardlink is a file.
            data(
                "usr/share/java/foo/hard.class",
                TestFile::from_bytes(CLASS).hardlink(),
            ),
            // Udebs are checked the same way.
            data("usr/lib/foo/Mini.class", TestFile::from_bytes(CLASS)).udeb(),
            // A non-UTF-8 name is still matched by its bytes.
            BinaryPackageCase::from(vec![(
                OsString::from_vec(b"usr/share/java/foo/\xff.class".to_vec()),
                TestFile::from_bytes(CLASS),
            )]),
        ];
        BinaryPackageTester::new(PackageInstallsJavaBytecode, pass, fail).test_and_snapshot();
    }
}
