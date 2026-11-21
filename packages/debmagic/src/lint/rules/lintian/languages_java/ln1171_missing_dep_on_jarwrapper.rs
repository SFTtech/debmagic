use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct MissingDepOnJarwrapper;

declare_rule! {
    /// The package has an executable jar and does not depend on `jarwrapper`.
    ///
    /// Covers the readable jars that lintian looks into that are regular files
    /// or hardlinks with an execute bit. `Pre-Depends` and `Depends` together
    /// must satisfy `jarwrapper`: a plain or versioned dependency does, an
    /// alternative (`foo | jarwrapper`) does not.
    /// Extra is empty. No Location; the Subject is the location.
    MissingDepOnJarwrapper,
    code = "LN1171",
    tag = "missing-dep-on-jarwrapper",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MissingDepOnJarwrapper {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(jars) = ctx.data_jars() else {
            return;
        };
        if !jars.inspected().any(|(_, entry)| entry.is_executable()) {
            return;
        }
        let satisfied = ctx
            .installation_control()
            .fields()
            .relations_and(&["Pre-Depends", "Depends"])
            .satisfies("jarwrapper");
        if !satisfied {
            ctx.diagnostic(Diagnostic::error(""));
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
            executable("usr/bin/a.jar", TestJar::new()).field("Depends", "jarwrapper, foo"),
            executable("usr/bin/a.jar", TestJar::new()).field("Pre-Depends", "jarwrapper"),
            // A version constraint does not make it a different package.
            executable("usr/bin/a.jar", TestJar::new()).field("Depends", "jarwrapper (>= 99)"),
            case("usr/share/java/a-1.0.jar", TestJar::new()),
            executable("usr/lib/jvm/java-21/lib/a.jar", TestJar::new()),
        ];
        let fail = vec![
            executable("usr/bin/testc.jar", TestJar::new()),
            executable("usr/bin/a.jar", TestJar::new()).field("Depends", "default-jre"),
            executable("usr/bin/a.jar", TestJar::new()).field("Recommends", "jarwrapper"),
            // An alternative does not guarantee it.
            executable("usr/bin/a.jar", TestJar::new()).field("Depends", "foo | jarwrapper"),
        ];
        BinaryPackageTester::new(MissingDepOnJarwrapper, pass, fail).test_and_snapshot();
    }
}
