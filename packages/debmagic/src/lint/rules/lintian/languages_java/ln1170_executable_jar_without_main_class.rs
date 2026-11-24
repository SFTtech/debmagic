use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct ExecutableJarWithoutMainClass;

declare_rule! {
    /// An executable jar has no `Main-Class` in its manifest.
    ///
    /// Covers the readable jars that lintian looks into that are regular files
    /// or hardlinks with an execute bit. A missing manifest, a missing
    /// `Main-Class`, an empty one, and `0` all count as no main class, as in
    /// Perl's truth. Extra is empty; the jar path is the Location.
    ExecutableJarWithoutMainClass,
    code = "LN1170",
    tag = "executable-jar-without-main-class",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ExecutableJarWithoutMainClass {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(jars) = ctx.data_jars() else {
            return;
        };
        for (path, entry) in jars.inspected() {
            let jar = entry.jar();
            if !entry.is_executable() || has_main_class(jar) {
                continue;
            }
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path: path.to_path_buf(),
                line: None,
                column: None,
            }));
        }
    }
}

fn has_main_class(jar: &crate::lint::jar::Jar) -> bool {
    jar.manifest()
        .and_then(|manifest| manifest.get("Main-Class"))
        .is_some_and(|main| !main.is_empty() && main != "0")
}

#[cfg(test)]
mod tests {
    use crate::lint::jar::TestJar;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(path: &str, jar: TestJar) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::from_bytes(jar.build()))])
    }

    fn executable(path: &str, jar: TestJar) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::from_bytes(jar.build()).mode(0o755))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            executable(
                "usr/bin/a.jar",
                TestJar::new().manifest("Main-Class: org.Main\n"),
            ),
            // Not executable.
            case(
                "usr/share/java/a-1.0.jar",
                TestJar::new().manifest("A: b\n"),
            ),
            case("usr/share/java/a-1.0.jar", TestJar::new()),
            // Inside a JVM.
            executable("usr/lib/jvm/java-21/lib/a.jar", TestJar::new()),
            BinaryPackageCase::from(vec![(
                "usr/bin/a.jar",
                TestFile::new("#!/bin/sh\n").mode(0o755),
            )]),
        ];
        let fail = vec![
            executable(
                "usr/bin/testc.jar",
                TestJar::new().manifest("Manifest-Version: 1.0\n"),
            ),
            executable("usr/bin/a.jar", TestJar::new()),
            executable("usr/bin/a.jar", TestJar::new().manifest("Main-Class: \n")),
            executable("usr/bin/a.jar", TestJar::new().manifest("Main-Class: 0\n")),
            // Only the first line of a header counts, so a main class on a
            // continuation line is not one.
            executable(
                "usr/bin/a.jar",
                TestJar::new().manifest("Main-Class:\n org.Main\n"),
            ),
            BinaryPackageCase::from(vec![(
                "usr/bin/a.jar",
                TestFile::from_bytes(TestJar::new().build())
                    .mode(0o755)
                    .hardlink(),
            )]),
        ];
        BinaryPackageTester::new(ExecutableJarWithoutMainClass, pass, fail).test_and_snapshot();
    }
}
