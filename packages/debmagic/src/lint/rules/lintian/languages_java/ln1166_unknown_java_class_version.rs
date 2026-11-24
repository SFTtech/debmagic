use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct UnknownJavaClassVersion;

declare_rule! {
    /// A jar holds a class file whose major version is outside the range lintian
    /// knows (45 to 71 in `data/java/constants`).
    ///
    /// Covers the readable jars that lintian looks into. Class members are
    /// walked in name order and the walk ends at the first unknown version, so
    /// one Diagnostic per jar. Class members without the `CAFEBABE` magic or
    /// shorter than eight bytes have no version and are skipped. Extra is
    /// `<class> -> <version>`; the jar path is the Location.
    UnknownJavaClassVersion,
    code = "LN1166",
    tag = "unknown-java-class-version",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for UnknownJavaClassVersion {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(jars) = ctx.data_jars() else {
            return;
        };
        for (path, entry) in jars.inspected() {
            let jar = entry.jar();
            let Some((class, version)) = jar.scan_classes().unknown_version else {
                continue;
            };
            ctx.diagnostic(
                Diagnostic::warn(format!("{class} -> {version}")).with_location(Location {
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
                TestJar::new().class("a/A.class", 45),
            ),
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().class("a/A.class", 71),
            ),
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().file("a/A.class", b"not a class file"),
            ),
            case(
                "usr/lib/jvm/java-21/lib/a.jar",
                TestJar::new().class("a/A.class", 42),
            ),
        ];
        let fail = vec![
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().class("org/debian/lintian/TestA.class", 42),
            ),
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().class("a/A.class", 44),
            ),
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().class("a/A.class", 72),
            ),
            // Only the first one per jar.
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new()
                    .class("a/A.class", 52)
                    .class("b/B.class", 99)
                    .class("c/C.class", 7),
            ),
        ];
        BinaryPackageTester::new(UnknownJavaClassVersion, pass, fail).test_and_snapshot();
    }
}
