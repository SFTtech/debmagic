use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::rule::BinaryPackageRule;

pub struct CodelessJar;

declare_rule! {
    /// A jar with a manifest has no class files.
    ///
    /// Covers the readable jars that lintian looks into that have a
    /// `META-INF/MANIFEST.MF` and no class-like member (`.class`, `.clj`,
    /// `.cljc`). Not reported for OSGi source bundles (`Bundle-SymbolicName`
    /// ending in `.source`), Maven javadoc jars under
    /// `usr/share/maven-repo/`, or Eclipse `.doc_*` / `.doc.user_*` /
    /// `.doc.isv_*` / `.source_*` jars, unless the manifest has a
    /// `Class-Path`. Extra is empty; the jar path is the Location.
    CodelessJar,
    code = "LN1168",
    tag = "codeless-jar",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CodelessJar {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(jars) = ctx.data_jars() else {
            return;
        };
        for (path, entry) in jars.inspected() {
            let jar = entry.jar();
            if !is_codeless(path, jar) {
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

static EXEMPT_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^usr/share/maven-repo/.*-javadoc\.jar|\.doc(?:\.(?:user|isv))?_[^/]+.jar|\.source_[^/]+.jar")
        .expect("exempt jar path regex")
});

fn is_codeless(path: &std::path::Path, jar: &crate::lint::jar::Jar) -> bool {
    let Some(manifest) = jar.manifest() else {
        return false;
    };
    if jar.scan_classes().has_classes {
        return false;
    }
    let class_path = manifest.get("Class-Path").unwrap_or_default();
    let source_bundle = manifest
        .get("Bundle-SymbolicName")
        .unwrap_or_default()
        .ends_with(".source");
    let exempt = source_bundle || EXEMPT_PATH.is_match(&path.to_string_lossy());
    !exempt || !class_path.is_empty()
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
        let manifest = "Manifest-Version: 1.0\n";
        let pass = vec![
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().manifest(manifest).class("a/A.class", 52),
            ),
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new()
                    .manifest(manifest)
                    .file("core.clj", b"(ns core)"),
            ),
            // No manifest is `missing-manifest` when there are classes, and
            // nothing at all otherwise.
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().file("data.txt", b"x"),
            ),
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().class("a/A.class", 52),
            ),
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new()
                    .manifest("Bundle-SymbolicName: org.foo.source\n")
                    .file("A.java", b"x"),
            ),
            case(
                "usr/share/maven-repo/org/foo/foo-1.0-javadoc.jar",
                TestJar::new().manifest(manifest).file("index.html", b"x"),
            ),
            case(
                "usr/share/eclipse/plugins/org.foo.doc_1.0.jar",
                TestJar::new().manifest(manifest),
            ),
            case(
                "usr/share/eclipse/plugins/org.foo.doc.user_1.0.jar",
                TestJar::new().manifest(manifest),
            ),
            case(
                "usr/share/eclipse/plugins/org.foo.source_1.0.jar",
                TestJar::new().manifest(manifest),
            ),
            case(
                "usr/lib/jvm/java-21/lib/a.jar",
                TestJar::new().manifest(manifest),
            ),
        ];
        let fail = vec![
            case(
                "usr/share/java/codeless-1.0.jar",
                TestJar::new().manifest(manifest).file("data.txt", b"x"),
            ),
            case("usr/share/java/empty-1.0.jar", TestJar::new().manifest("")),
            // A class path makes even an exempt jar codeless.
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().manifest("Bundle-SymbolicName: org.foo.source\nClass-Path: b.jar\n"),
            ),
            case(
                "usr/share/maven-repo/org/foo/foo-1.0-javadoc.jar",
                TestJar::new().manifest("Class-Path: b.jar\n"),
            ),
            case(
                "usr/share/eclipse/plugins/org.foo.doc_x/1.jar",
                TestJar::new().manifest(manifest),
            ),
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().manifest(manifest).file("x/y.png", b"x"),
            ),
        ];
        BinaryPackageTester::new(CodelessJar, pass, fail).test_and_snapshot();
    }
}
