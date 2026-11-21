use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct ZipParseError;

declare_rule! {
    /// A file named `*.jar` that starts like a zip archive cannot be read as one.
    ///
    /// Covers the Data files regular files and hardlinks named `*.jar` (in any
    /// letter case) whose bytes start with `PK\x03\x04`, or `PK\x05\x06` for an
    /// empty archive, as `file(1)` calls a zip archive. Extra is
    /// `Archive::Zip`'s error, for example `format error: can't find EOCD
    /// signature`. The reader is this crate's own (`lint::zip`), so errors
    /// that end in the host's `$!` stop at the colon and a few messages
    /// differ. Jars inside a JVM or Mozilla directory are reported too, as in
    /// lintian. The jar path is the Location.
    ZipParseError,
    code = "LN1163",
    tag = "zip-parse-error",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ZipParseError {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(jars) = ctx.data_jars() else {
            return;
        };
        for (path, entry) in jars.iter() {
            let jar = entry.jar();
            let Some(error) = jar.error() else {
                continue;
            };
            ctx.diagnostic(Diagnostic::warn(error).with_location(Location {
                path: path.to_path_buf(),
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::jar::TestJar;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(path: &str, jar: TestJar) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::from_bytes(jar.build()))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().class("a/A.class", 52),
            ),
            // Not a zip archive by its first bytes: `file(1)` says data.
            BinaryPackageCase::from(vec![(
                "usr/share/java/a-1.0.jar",
                TestFile::new("not a zip"),
            )]),
            BinaryPackageCase::from(vec![("usr/share/java/a-1.0.jar", TestFile::new(""))]),
            // Not named `.jar`.
            BinaryPackageCase::from(vec![(
                "usr/share/java/a-1.0.zip",
                TestFile::from_bytes(b"PK\x03\x04 truncated".to_vec()),
            )]),
        ];
        let truncated = TestJar::new().class("a/A.class", 52).build();
        let truncated = &truncated[..truncated.len() - 25];
        let fail = vec![
            BinaryPackageCase::from(vec![(
                "usr/share/java/a-1.0.jar",
                TestFile::from_bytes(truncated.to_vec()),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/java/A.JAR",
                TestFile::from_bytes(truncated.to_vec()),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/lib/jvm/java-21/lib/a.jar",
                TestFile::from_bytes(truncated.to_vec()),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/java/a-1.0.jar",
                TestFile::from_bytes(b"PK\x03\x04 tiny".to_vec()),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/share/java/a-1.0.jar",
                TestFile::from_bytes(truncated.to_vec()).hardlink(),
            )]),
        ];
        BinaryPackageTester::new(ZipParseError, pass, fail).test_and_snapshot();
    }
}
