use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct ClojurePackageButNotDhClojure;

declare_rule! {
    /// A Clojure source package, whose source name ends in `-clojure`, build-depends
    /// on `leiningen` but not on `dh-clojure`. Reported with an empty extra and no
    /// Location, since the Subject is the location. Applies to the Source tree and
    /// Source package. Experimental, as lintian marks the Tag.
    ///
    /// `Build-Depends-All` is the `Build-Depends`, `Build-Depends-Indep` and
    /// `Build-Depends-Arch` fields of the source paragraph, AND-ed, as lintian's
    /// `relation('Build-Depends-All')` does.
    ClojurePackageButNotDhClojure,
    code = "LN1398",
    tag = "clojure-package-but-not-dh-clojure",
    default_selected = false,
    experimental = true,
}

const BUILD_DEPENDS: [&str; 3] = ["Build-Depends", "Build-Depends-Indep", "Build-Depends-Arch"];

impl SourceRule for ClojurePackageButNotDhClojure {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let Some(name) = control.source().get("Source") else {
            return;
        };
        if !name.ends_with("-clojure") {
            return;
        }
        let build = control.source().relations_and(&BUILD_DEPENDS);
        if build.satisfies("leiningen") && !build.satisfies("dh-clojure") {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(source: &str, build_depends: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: {source}\n\
                 Maintainer: Example <ex@example.com>\n\
                 Standards-Version: 4.7.2\n\
                 Build-Depends: {build_depends}\n\
                 \n\
                 Package: example\n\
                 Architecture: all\n\
                 Description: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            control("example", "leiningen"),
            control("example-clojure", "debhelper-compat (= 13)"),
            control("example-clojure", "leiningen, dh-clojure"),
            control("example-clojure", "dh-clojure"),
            vec![("README", TestFile::new("no control"))],
        ];
        let fail = vec![
            control("example-clojure", "leiningen"),
            control("example-clojure", "leiningen (>= 2.0)"),
        ];
        SourceTreeTester::new(ClojurePackageButNotDhClojure, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(control("example", "leiningen"))];
        let fail = vec![SourcePackageCase::from(control(
            "example-clojure",
            "leiningen",
        ))];
        SourcePackageTester::new(ClojurePackageButNotDhClojure, pass, fail).test_and_snapshot();
    }
}
