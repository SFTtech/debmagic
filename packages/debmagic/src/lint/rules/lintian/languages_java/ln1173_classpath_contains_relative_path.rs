use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::lint::rule::BinaryPackageRule;

pub struct ClasspathContainsRelativePath;

declare_rule! {
    /// A jar's manifest `Class-Path` names a relative path with a directory
    /// that does not lead to a file or symlink in the package.
    ///
    /// Covers the readable jars that lintian looks into with a non-empty
    /// `Class-Path` (only its first line, as lintian reads it). Each
    /// whitespace-separated path is stripped of a leading `./`; one that does
    /// not start with `/` or `file:///` and contains a `/` is resolved
    /// against the jar's directory. It is skipped when that goes above the
    /// package root, when it is a `usr/share/java/<name>.jar` and the package
    /// depends on a `lib*-java` package, and when it names a file, hardlink
    /// or symlink of the package. Extra is the remaining paths joined with
    /// `, `; the jar path is the Location.
    ClasspathContainsRelativePath,
    code = "LN1173",
    tag = "classpath-contains-relative-path",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ClasspathContainsRelativePath {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(jars) = ctx.data_jars() else {
            return;
        };
        let depends = ctx
            .installation_control()
            .fields()
            .relations_and(&["Pre-Depends", "Depends"])
            .to_lintian_string();
        let java_library_dependency = depends_on_java_library(&depends);
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut hits = Vec::new();
        for (path, entry) in jars.inspected() {
            let Some(class_path) = entry
                .jar()
                .manifest()
                .and_then(|manifest| manifest.get("Class-Path"))
                .filter(|class_path| !class_path.is_empty())
            else {
                continue;
            };
            let directory = path.parent().unwrap_or(Path::new(""));
            let relative: Vec<&str> = class_path
                .split_whitespace()
                .map(|p| p.trim_start_matches("./"))
                .filter(|p| !p.is_empty())
                .filter(|p| is_relative_with_directory(p))
                .filter(|p| {
                    let Some(target) = normalize_link_target(directory, p) else {
                        return false;
                    };
                    if java_library_dependency && SHARED_JAR.is_match(&target.to_string_lossy()) {
                        return false;
                    }
                    !files
                        .get(&target)
                        .is_some_and(|entry| entry.is_file() || entry.is_symlink())
                })
                .collect();
            if !relative.is_empty() {
                hits.push((path.to_path_buf(), relative.join(", ")));
            }
        }
        for (path, extra) in hits {
            ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

static SHARED_JAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^usr/share/java/[^/]+.jar$").expect("shared jar regex"));

static ABSOLUTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:file://)?/").expect("absolute path regex"));

/// Not absolute, and with a directory part.
fn is_relative_with_directory(path: &str) -> bool {
    !ABSOLUTE.is_match(path) && path.contains('/')
}

/// The relation text names a `lib*-java` package, ignoring `lib*-java-doc`:
/// lintian removes those and matches `\b(lib[^\s,]+-java)\b`.
fn depends_on_java_library(depends: &str) -> bool {
    static DOC: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"lib[^\s,]+-java-doc").expect("doc package regex"));
    static LIBRARY: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\blib[^\s,]+-java\b").expect("library regex"));
    LIBRARY.is_match(&DOC.replace_all(depends, ""))
}

/// Lintian's `normalize_link_target`: `target` against `directory`, without
/// `.` and empty components; `None` when it climbs above the package root.
fn normalize_link_target(directory: &Path, target: &str) -> Option<PathBuf> {
    let joined = directory.join(target);
    let mut out = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
        }
    }
    Some(out)
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
        let class_path = |text: &str| TestJar::new().manifest(&format!("Class-Path: {text}\n"));
        let jar = |path: &str, text: &str| case(path, class_path(text));
        let pass = vec![
            // No directory part, absolute, or empty.
            jar("usr/share/java/a-1.0.jar", "b.jar c.jar"),
            jar(
                "usr/share/java/a-1.0.jar",
                "/usr/share/java/b.jar file:///usr/share/java/c.jar",
            ),
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().manifest("Class-Path: \n"),
            ),
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().manifest("Main-Class: A\n"),
            ),
            case("usr/share/java/a-1.0.jar", TestJar::new()),
            // Climbing out of the package.
            jar("usr/share/java/a-1.0.jar", "../../../../../x/y.jar"),
            // A file in the package.
            BinaryPackageCase::from(vec![
                (
                    "usr/share/java/a-1.0.jar",
                    TestFile::from_bytes(class_path("../lib/b.jar").build()),
                ),
                ("usr/share/lib/b.jar", TestFile::new("x")),
            ]),
            // A symlink in the package, even a dangling one.
            BinaryPackageCase::from(vec![
                (
                    "usr/share/java/a-1.0.jar",
                    TestFile::from_bytes(class_path("./sub/b.jar").build()),
                ),
                ("usr/share/java/sub/b.jar", TestFile::symlink("/nowhere")),
            ]),
            // `usr/share/java/*.jar` with a Java library dependency.
            jar("usr/share/java/a-1.0.jar", "../java/b.jar").field("Depends", "libfoo-java"),
            // Continuation lines are not read.
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().manifest("Class-Path: b.jar\n sub/c.jar\n"),
            ),
        ];
        let fail = vec![
            jar("usr/share/java/a-1.0.jar", "obviously-relative/stuff.jar"),
            jar(
                "usr/share/java/a-1.0.jar",
                "./sub/b.jar ok.jar ../other/c.jar",
            ),
            jar("usr/share/java/a-1.0.jar", "../java/b.jar"),
            jar("usr/share/java/a-1.0.jar", "../java/b.jar").field("Depends", "libfoo-java-doc"),
            jar("usr/share/java/a-1.0.jar", "../java/b.jar").field("Depends", "foo-java"),
            jar("usr/share/java/a-1.0.jar", "../lib/b.jar").field("Depends", "libfoo-java"),
            // A directory is not a file or symlink.
            BinaryPackageCase::from(vec![
                (
                    "usr/share/java/a-1.0.jar",
                    TestFile::from_bytes(class_path("sub/b.jar").build()),
                ),
                ("usr/share/java/sub/b.jar/x", TestFile::new("x")),
            ]),
        ];
        BinaryPackageTester::new(ClasspathContainsRelativePath, pass, fail).test_and_snapshot();
    }
}
