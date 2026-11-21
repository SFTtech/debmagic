use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::jar::BytecodeLimits;
use crate::lint::rule::BinaryPackageRule;

pub struct IncompatibleJavaBytecodeFormat;

declare_rule! {
    /// The lowest class-file version among the package's jars is newer than
    /// the Debian default JVM targets (`default-bytecode-version` in lintian's
    /// `data/java/constants`).
    ///
    /// Covers every readable jar that lintian looks into: each contributes the
    /// lowest known version of its classes (a jar's walk ends at an unknown
    /// version, whose class is not counted), and the minimum over the package
    /// is compared. Extra is `Java<N> version (Class format: <version>)` with
    /// N the version minus 44. No Location; the Subject is the location.
    ///
    /// The one lintian recipe (`java-class-format`) expects this Tag at
    /// format 63, which the vendored table's default of 69 does not report;
    /// it also needs a JDK, which the Parity host lacks.
    IncompatibleJavaBytecodeFormat,
    code = "LN1167",
    tag = "incompatible-java-bytecode-format",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for IncompatibleJavaBytecodeFormat {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(jars) = ctx.data_jars() else {
            return;
        };
        let lowest = jars
            .inspected()
            .filter_map(|(_, entry)| entry.jar().scan_classes().lowest_version)
            .min();
        let Some(lowest) = lowest else {
            return;
        };
        if lowest > BytecodeLimits::vendored().default {
            let java = lowest - 44;
            ctx.diagnostic(Diagnostic::warn(format!(
                "Java{java} version (Class format: {lowest})"
            )));
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
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().class("a/A.class", 69),
            ),
            // The lowest version of the package decides.
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().class("a/A.class", 52).class("b/B.class", 70),
            ),
            // Unknown versions are not counted, and a JVM's jars are not read.
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().class("a/A.class", 99),
            ),
            case(
                "usr/lib/jvm/java-21/lib/a.jar",
                TestJar::new().class("a/A.class", 70),
            ),
            BinaryPackageCase::from(vec![(
                "usr/share/java/a-1.0.jar",
                TestFile::new("not a zip"),
            )]),
        ];
        let fail = vec![
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().class("a/A.class", 70),
            ),
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().class("a/A.class", 71).class("b/B.class", 70),
            ),
            BinaryPackageCase::from(vec![
                (
                    "usr/share/java/a-1.0.jar",
                    TestFile::from_bytes(TestJar::new().class("a/A.class", 71).build()),
                ),
                (
                    "usr/share/java/b-1.0.jar",
                    TestFile::from_bytes(TestJar::new().class("b/B.class", 70).build()),
                ),
            ]),
        ];
        BinaryPackageTester::new(IncompatibleJavaBytecodeFormat, pass, fail).test_and_snapshot();
    }
}
