use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct JavalibButNoPublicJars;

declare_rule! {
    /// A `lib*-java` package ships no jar in `usr/share/java/`.
    ///
    /// The name suggests a Java library, but no installed `usr/share/java/<name>.jar`
    /// reads as a zip archive without error. Only a jar that lintian can read
    /// counts, so a file that is not a zip, or a jar that fails to parse, does
    /// not. Not reported for a transitional package (a Description that
    /// contains `transitional package`, in any case). Lintian's symlink
    /// exception (a `usr/share/java/` symlink installed by the package) never
    /// applies: its check compares the full child names with `^[^/]+\.jar$`, so
    /// it matches nothing, and the Rule keeps that. Reported as an info hint
    /// with no Extra and no Location. Binary packages only; udebs are checked
    /// too.
    JavalibButNoPublicJars,
    code = "LN1744",
    tag = "javalib-but-no-public-jars",
    default_selected = false,
    experimental = false,
}

static JAVA_LIBRARY_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^lib[^\s,]+-java$").expect("valid regex"));
static PUBLIC_JAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/share/java/[^/]+\.jar$").expect("valid regex"));
static TRANSITIONAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)transitional package").expect("valid regex"));

impl BinaryPackageRule for JavalibButNoPublicJars {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if !JAVA_LIBRARY_NAME.is_match(ctx.package_name()) {
            return;
        }
        let transitional = TRANSITIONAL.is_match(
            ctx.installation_control()
                .fields()
                .get("Description")
                .unwrap_or_default(),
        );
        if transitional {
            return;
        }
        let has_public_jars = ctx.data_jars().is_some_and(|jars| {
            jars.inspected().any(|(path, entry)| {
                PUBLIC_JAR.is_match(&path.to_string_lossy()) && entry.jar().error().is_none()
            })
        });
        if !has_public_jars {
            ctx.diagnostic(Diagnostic::warn("").with_severity(Severity::Info));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::jar::TestJar;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn jar() -> TestFile {
        TestFile::from_bytes(TestJar::new().build())
    }

    fn library(name: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).package(name)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            // Not a Java library name.
            library("libfoo").field("Description", "x"),
            library("foo-java"),
            // A public jar.
            BinaryPackageCase::from(vec![("usr/share/java/libfoo-1.0.jar", jar())])
                .package("libfoo-java"),
            // A transitional package.
            library("libfoo-java").field("Description", "Transitional package for libfoo"),
            // A jar that is not a zip does not count, but the package is not
            // reported when it is transitional.
            BinaryPackageCase::from(vec![(
                "usr/share/java/libfoo.jar",
                TestFile::new("not a jar"),
            )])
            .package("libfoo-java")
            .field("Description", "Transitional package"),
        ];
        let fail = vec![
            library("libfoo-java"),
            // A jar outside usr/share/java is for private use.
            BinaryPackageCase::from(vec![("usr/lib/foo/libfoo.jar", jar())]).package("libfoo-java"),
            // A jar that is not readable as a zip.
            BinaryPackageCase::from(vec![(
                "usr/share/java/libfoo.jar",
                TestFile::new("not a jar"),
            )])
            .package("libfoo-java"),
            library("libfoo-java").udeb(),
        ];
        BinaryPackageTester::new(JavalibButNoPublicJars, pass, fail).test_and_snapshot();
    }
}
