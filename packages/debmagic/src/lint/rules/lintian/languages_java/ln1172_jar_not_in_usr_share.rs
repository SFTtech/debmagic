use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct JarNotInUsrShare;

declare_rule! {
    /// A jar that is not executable is installed outside `usr/share/`.
    ///
    /// Covers the readable jars that lintian looks into that are not regular
    /// files or hardlinks with an execute bit, with a path that does not
    /// start with `usr/share/`. Extra is empty; the jar path is the
    /// Location.
    JarNotInUsrShare,
    code = "LN1172",
    tag = "jar-not-in-usr-share",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for JarNotInUsrShare {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(jars) = ctx.data_jars() else {
            return;
        };
        for (path, entry) in jars.inspected() {
            if entry.is_executable() || path.starts_with("usr/share") {
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
            case("usr/share/java/a-1.0.jar", TestJar::new()),
            case("usr/share/foo/a.jar", TestJar::new()),
            executable("usr/bin/a.jar", TestJar::new()),
            case("usr/lib/jvm/java-21/lib/a.jar", TestJar::new()),
            BinaryPackageCase::from(vec![("usr/lib/a.jar", TestFile::new("not a jar"))]),
        ];
        let fail = vec![
            case("usr/lib/testb.jar", TestJar::new()),
            case("opt/foo/a.jar", TestJar::new()),
            case("usr/sharefoo/a.jar", TestJar::new()),
            case("usr/lib/jvm/direct.jar", TestJar::new()),
        ];
        BinaryPackageTester::new(JarNotInUsrShare, pass, fail).test_and_snapshot();
    }
}
