use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::BINARY_RELATION_FIELDS;

pub struct NeedlessDependencyOnJre;

declare_rule! {
    /// A Java library depends only on Java runtimes or development kits.
    ///
    /// The package name matches `^lib.*-java$`, and a dependency in
    /// `Depends` consists entirely of Java runtime or JDK packages (the
    /// `default-jre`, `java-runtime`, `openjdk-N-jre`, `gcj`, and similar
    /// families of `fields/package-relations`). Reported once per package,
    /// with no extra. Applies to debs and udebs.
    NeedlessDependencyOnJre,
    code = "LN0939",
    tag = "needless-dependency-on-jre",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for NeedlessDependencyOnJre {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let needless = {
            let fields = ctx.installation_control().fields();
            is_needless(&package, fields)
        };
        if needless {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

/// lintian's walk over the relationship fields. A dependency whose alternative
/// count equals the Java alternatives counted so far in its field is reported,
/// so an empty dependency also matches before any Java alternative.
fn is_needless(package: &str, fields: &ControlParagraph) -> bool {
    let javalib = package.starts_with("lib") && package.ends_with("-java");
    BINARY_RELATION_FIELDS.iter().any(|field| {
        let Some(relation) = fields.relation(field) else {
            return false;
        };
        let mut java_count = 0;
        relation.dependencies().iter().any(|dependency| {
            for alternative in dependency {
                if javalib
                    && *field == "Depends"
                    && JAVA_PACKAGES
                        .iter()
                        .any(|pattern| pattern.is_match(alternative.package()))
                {
                    java_count += 1;
                }
            }
            dependency.len() == java_count
        })
    })
}

static JAVA_PACKAGES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        "default-j(?:re|dk)(?:-headless)?",
        "java\\d*-runtime(?:-headless)?",
        "(openjdk-|sun-java)\\d+-j(?:re|dk)(?:-headless)?",
        "gcj-(?:\\d+\\.\\d+-)?jre(?:-headless)?",
        "gcj-(?:\\d+\\.\\d+-)?jdk",
        "gij",
        "java-gcj-compat(?:-dev|-headless)?",
        "kaffe",
        "cacao",
        "jamvm",
        "classpath",
    ]
    .iter()
    .map(|pattern| Regex::new(pattern).expect("valid regex"))
    .collect()
});

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn fields(items: &[(&str, &str)]) -> BinaryPackageCase {
        items.iter().fold(
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            |case, (name, value)| case.field(*name, *value),
        )
    }

    #[test]
    fn test() {
        let pass = vec![
            fields(&[]),
            fields(&[("Depends", "default-jre | libc6")]).package("libfoo-java"),
            fields(&[("Depends", "openjdk-17-jre-headless")]).package("foo"),
            fields(&[("Recommends", "default-jre")]).package("libfoo-java"),
            fields(&[("Depends", "libc6, libfoo")]).package("libfoo-java"),
        ];
        let fail = vec![
            fields(&[("Depends", "default-jre")]).package("libfoo-java"),
            fields(&[("Depends", "libc6, default-jre")]).package("libfoo-java"),
            fields(&[("Depends", "openjdk-17-jre-headless, java-runtime")]).package("lib-java"),
            fields(&[("Depends", "java-runtime-headless | gij")]).package("libfoo-java"),
        ];
        BinaryPackageTester::new(NeedlessDependencyOnJre, pass, fail).test_and_snapshot();
    }
}
