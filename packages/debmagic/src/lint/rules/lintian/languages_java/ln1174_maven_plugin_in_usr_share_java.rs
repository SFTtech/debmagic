use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::rule::BinaryPackageRule;

pub struct MavenPluginInUsrShareJava;

declare_rule! {
    /// A Maven plugin package installs a jar in `usr/share/java/` and others
    /// outside the Maven repository.
    ///
    /// Applies to packages named `lib*maven*plugin*`. Lintian walks the jars
    /// it looks into in path order; once a jar directly in `usr/share/java/`
    /// (`usr/share/java/<name>.jar`) has been seen, itself included, every
    /// jar not under `usr/share/maven-repo/` is reported. Extra is empty; the
    /// jar path is the Location.
    MavenPluginInUsrShareJava,
    code = "LN1174",
    tag = "maven-plugin-in-usr-share-java",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MavenPluginInUsrShareJava {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if !PLUGIN_PACKAGE.is_match(ctx.package_name()) {
            return;
        }
        let Some(jars) = ctx.data_jars() else {
            return;
        };
        let mut has_public_jars = false;
        let mut hits = Vec::new();
        for (path, _) in jars.inspected() {
            let name = path.to_string_lossy();
            if PUBLIC_JAR.is_match(&name) {
                has_public_jars = true;
            }
            if has_public_jars && !MAVEN_REPO_JAR.is_match(&name) {
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

static PLUGIN_PACKAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^lib.*maven.*plugin.*").expect("plugin package regex"));
static PUBLIC_JAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/share/java/[^/]+\.jar$").expect("public jar regex"));
static MAVEN_REPO_JAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/share/maven-repo/.*\.jar").expect("maven repo regex"));

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
        let plugin = |path: &str| case(path, TestJar::new()).package("libtest-maven-plugin-java");
        let pass = vec![
            // Not a plugin package.
            case("usr/share/java/testb-1.0.jar", TestJar::new()),
            case("usr/share/java/testb-1.0.jar", TestJar::new()).package("libfoo-java"),
            // No public jar.
            plugin("usr/share/maven-repo/org/foo/foo/1.0/foo-1.0.jar"),
            plugin("usr/lib/foo.jar"),
            // A jar before the first public one is not reported.
            BinaryPackageCase::from(vec![
                (
                    "usr/lib/a.jar",
                    TestFile::from_bytes(TestJar::new().build()),
                ),
                (
                    "usr/share/maven-repo/b.jar",
                    TestFile::from_bytes(TestJar::new().build()),
                ),
            ])
            .package("libfoo-maven-plugin-java"),
        ];
        let fail = vec![
            // The Maven repository is where a jar belongs: only the public one
            // is reported.
            BinaryPackageCase::from(vec![
                (
                    "usr/share/java/foo.jar",
                    TestFile::from_bytes(TestJar::new().build()),
                ),
                (
                    "usr/share/maven-repo/org/foo/foo/1.0/foo-1.0.jar",
                    TestFile::from_bytes(TestJar::new().build()),
                ),
            ])
            .package("libfoo-maven-plugin-java"),
            plugin("usr/share/java/testb-1.0.jar"),
            plugin("usr/share/java/testb.jar"),
            BinaryPackageCase::from(vec![
                (
                    "usr/share/java/foo.jar",
                    TestFile::from_bytes(TestJar::new().build()),
                ),
                (
                    "usr/share/java/sub/bar.jar",
                    TestFile::from_bytes(TestJar::new().build()),
                ),
            ])
            .package("libfoo-maven-plugin-java"),
            // After a public jar, a jar anywhere else is reported (sorted by path).
            BinaryPackageCase::from(vec![
                (
                    "usr/share/java/foo.jar",
                    TestFile::from_bytes(TestJar::new().build()),
                ),
                (
                    "usr/share/zzz/bar.jar",
                    TestFile::from_bytes(TestJar::new().build()),
                ),
            ])
            .package("libfoo-maven-plugin-java"),
        ];
        BinaryPackageTester::new(MavenPluginInUsrShareJava, pass, fail).test_and_snapshot();
    }
}
