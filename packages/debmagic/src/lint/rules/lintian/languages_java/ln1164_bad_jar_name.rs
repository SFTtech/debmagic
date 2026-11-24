use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::rule::BinaryPackageRule;

pub struct BadJarName;

declare_rule! {
    /// A jar directly in `usr/share/java/` is not named
    /// `<package>-<version>.jar`, as the Java Policy asks (Debian bug 976681).
    ///
    /// Covers the readable jars that lintian looks into (not inside a JVM or
    /// Mozilla directory) with a path of the form `usr/share/java/<name>.jar`.
    /// The name must match lintian's `$PKGNAME_REGEX`, a hyphen, and
    /// `$PKGVERSION_REGEX` before `.jar`. Extra is empty; the jar path is the
    /// Location.
    BadJarName,
    code = "LN1164",
    tag = "bad-jar-name",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BadJarName {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(jars) = ctx.data_jars() else {
            return;
        };
        for (path, _) in jars.inspected() {
            if !in_usr_share_java(path) || is_well_named(path) {
                continue;
            }
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: path.to_path_buf(),
                line: None,
                column: None,
            }));
        }
    }
}

static IN_USR_SHARE_JAVA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/share/java/[^/]+\.jar$").expect("jar path regex"));

/// `$PKGNAME_REGEX-$PKGVERSION_REGEX\.jar` over the whole file name.
static WELL_NAMED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z0-9][-+.a-z0-9]+-(?:\d+:)?[0-9][0-9A-Za-z.+:~]*(?:-[0-9A-Za-z.+:~]+)*\.jar$")
        .expect("jar name regex")
});

fn in_usr_share_java(path: &std::path::Path) -> bool {
    IN_USR_SHARE_JAVA.is_match(&path.to_string_lossy())
}

fn is_well_named(path: &std::path::Path) -> bool {
    path.file_name()
        .is_some_and(|name| WELL_NAMED.is_match(&name.to_string_lossy()))
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
        let jar = || TestJar::new().class("a/A.class", 52);
        let pass = vec![
            case("usr/share/java/foo-1.0.jar", jar()),
            case("usr/share/java/libfoo-java-1:2.3.4-5~bpo1.jar", jar()),
            case("usr/share/java/foo-bar-1.0-rc1.jar", jar()),
            // Not directly in `usr/share/java/`, or not a jar there.
            case("usr/share/java/sub/foo.jar", jar()),
            case("usr/share/maven-repo/foo.jar", jar()),
            case("usr/lib/foo.jar", jar()),
            // The extension is case-sensitive here.
            case("usr/share/java/F.JAR", jar()),
            // Inside a JVM.
            case("usr/lib/jvm/java-21/lib/foo.jar", jar()),
        ];
        let fail = vec![
            case("usr/share/java/foo.jar", jar()),
            case("usr/share/java/0.jar", jar()),
            case("usr/share/java/Foo-1.0.jar", jar()),
            case("usr/share/java/foo-1.0-.jar", jar()),
            case("usr/share/java/foo_1.0.jar", jar()),
            case("usr/share/java/-1.0.jar", jar()),
        ];
        BinaryPackageTester::new(BadJarName, pass, fail).test_and_snapshot();
    }
}
