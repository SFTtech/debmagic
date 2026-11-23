use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::jar::JarFiles;
use crate::lint::rule::{Severity, SourceRule};

pub struct SourceContainsPrebuiltJavaObject;

declare_rule! {
    /// A Patched file is a jar that holds a prebuilt Java class (a `.class`
    /// member, or a Clojure `.clj` or `.cljc` one), which developers often
    /// leave in a tarball by mistake. Reported at pedantic severity with an
    /// empty extra and the path of the jar as the Location, the pointer form
    /// lintian's `languages/java` Check emits.
    ///
    /// A jar is a regular file or hardlink named `*.jar` (in any letter case)
    /// that reads as a zip archive. A jar that does not read is not reported.
    /// Quilt's `.pc/` is skipped.
    ///
    /// Differs from lintian: the jar's members come from the zip central
    /// directory, read here, not from `jar -tf`.
    SourceContainsPrebuiltJavaObject,
    code = "LN1626",
    tag = "source-contains-prebuilt-java-object",
    default_selected = false,
    experimental = false,
}

impl SourceRule for SourceContainsPrebuiltJavaObject {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let flagged: Vec<PathBuf> = JarFiles::from_tree(files)
            .iter()
            .filter(|(path, _)| !path.starts_with(".pc"))
            .filter(|(_, jar)| {
                let jar = jar.jar();
                jar.error().is_none() && jar.scan_classes().has_classes
            })
            .map(|(path, _)| path.to_path_buf())
            .collect();
        for path in flagged {
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

#[cfg(test)]
mod tests {
    use crate::lint::jar::TestJar;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    fn with_class() -> Vec<u8> {
        TestJar::new()
            .manifest("Manifest-Version: 1.0\n")
            .class("HelloWorld.class", 52)
            .build()
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/control", TestFile::from(CONTROL))],
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                // A jar of resources only.
                (
                    "lib/resources.jar",
                    TestFile::from_bytes(
                        TestJar::new()
                            .manifest("Manifest-Version: 1.0\n")
                            .file("data/notes.txt", b"hello")
                            .build(),
                    ),
                ),
                // A zip that is not a jar is not this Tag.
                ("lib/archive.zip", TestFile::from_bytes(with_class())),
                // Quilt's `.pc/` is not the packaging's file.
                (".pc/HelloWorld.jar", TestFile::from_bytes(with_class())),
                // A jar that is not a zip archive cannot be read.
                ("lib/broken.jar", TestFile::new("not a zip archive\n")),
            ],
        ];
        let fail = vec![
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                ("HelloWorld.jar", TestFile::from_bytes(with_class())),
            ],
            // A Clojure source in a jar is a class-like member too.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "lib/core.jar",
                    TestFile::from_bytes(TestJar::new().file("core.CLJ", b"(ns core)").build()),
                ),
            ],
            // A hardlink is a file too.
            vec![
                ("debian/control", TestFile::from(CONTROL)),
                (
                    "HelloWorld.jar",
                    TestFile::from_bytes(with_class()).hardlink(),
                ),
            ],
        ];
        SourceTreeTester::new(SourceContainsPrebuiltJavaObject, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(vec![(
            "debian/control",
            TestFile::from(CONTROL),
        )])];
        let fail = vec![SourcePackageCase::from(vec![
            ("debian/control", TestFile::from(CONTROL)),
            ("HelloWorld.jar", TestFile::from_bytes(with_class())),
        ])];
        SourcePackageTester::new(SourceContainsPrebuiltJavaObject, pass, fail).test_and_snapshot();
    }
}
