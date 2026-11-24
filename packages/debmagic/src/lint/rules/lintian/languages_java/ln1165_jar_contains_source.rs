use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct JarContainsSource;

declare_rule! {
    /// A jar ships the `.java` source next to a compiled class (or Clojure
    /// file) of the same name.
    ///
    /// Covers the readable jars that lintian looks into. Class-like members
    /// (`.class`, `.clj`, `.cljc`, in any letter case) are walked in name order;
    /// the walk ends at the first class whose version is outside lintian's
    /// known range (`unknown-java-class-version`), so sources next to later
    /// classes are not reported. Extra is the source member name, for example
    /// `org/debian/lintian/TestA.java`; the jar path is the Location.
    JarContainsSource,
    code = "LN1165",
    tag = "jar-contains-source",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for JarContainsSource {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(jars) = ctx.data_jars() else {
            return;
        };
        for (path, entry) in jars.inspected() {
            for source in entry.jar().scan_classes().sources {
                ctx.diagnostic(Diagnostic::warn(source).with_location(Location {
                    path: path.to_path_buf(),
                    line: None,
                    column: None,
                }));
            }
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
            // The source has no class.
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().file("a/A.java", b"class A {}"),
            ),
            // A different directory.
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().class("a/A.class", 52).file("b/A.java", b"x"),
            ),
            // The walk ended at the unknown class `a/B.class`.
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new()
                    .class("a/B.class", 42)
                    .class("a/C.class", 52)
                    .file("a/C.java", b"x"),
            ),
            case(
                "usr/lib/jvm/java-21/lib/a.jar",
                TestJar::new().class("a/A.class", 52).file("a/A.java", b"x"),
            ),
        ];
        let fail = vec![
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().class("a/A.class", 52).file("a/A.java", b"x"),
            ),
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new()
                    .class("a/A.class", 52)
                    .file("a/A.java", b"x")
                    .class("a/B.class", 52)
                    .file("a/B.java", b"x"),
            ),
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new()
                    .file("core.clj", b"(ns core)")
                    .file("core.java", b"x"),
            ),
            // Sources before the unknown class are still found.
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new()
                    .class("a/A.class", 52)
                    .file("a/A.java", b"x")
                    .class("b/B.class", 42),
            ),
        ];
        BinaryPackageTester::new(JarContainsSource, pass, fail).test_and_snapshot();
    }
}
