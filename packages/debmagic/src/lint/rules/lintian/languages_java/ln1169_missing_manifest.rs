use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct MissingManifest;

declare_rule! {
    /// A jar has class files but no manifest.
    ///
    /// Covers the readable jars that lintian looks into that have a class-like
    /// member (`.class`, `.clj`, `.cljc`) and no `META-INF/MANIFEST.MF`
    /// (found in any letter case). Info severity and not selected by default,
    /// as in lintian. Extra is empty; the jar path is the Location.
    MissingManifest,
    code = "LN1169",
    tag = "missing-manifest",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for MissingManifest {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(jars) = ctx.data_jars() else {
            return;
        };
        for (path, entry) in jars.inspected() {
            let jar = entry.jar();
            if jar.manifest().is_some() || !jar.scan_classes().has_classes {
                continue;
            }
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path: path.to_path_buf(),
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
                TestJar::new().manifest("A: b\n").class("a/A.class", 52),
            ),
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().file("data.txt", b"x"),
            ),
            case("usr/share/java/a-1.0.jar", TestJar::new()),
            case(
                "usr/lib/jvm/java-21/lib/a.jar",
                TestJar::new().class("a/A.class", 52),
            ),
        ];
        let fail = vec![
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().class("a/A.class", 52),
            ),
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().file("core.clj", b"(ns core)"),
            ),
            // The manifest must be at the top level of the jar.
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new()
                    .class("a/A.class", 52)
                    .file("x/META-INF/MANIFEST.MF", b"A: b\n"),
            ),
        ];
        BinaryPackageTester::new(MissingManifest, pass, fail).test_and_snapshot();
    }
}
