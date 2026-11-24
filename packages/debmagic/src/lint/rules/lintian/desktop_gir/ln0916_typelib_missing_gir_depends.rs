use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct TypelibMissingGirDepends;

declare_rule! {
    /// A `gir<version>-*` binary package does not depend on `${gir:Depends}`.
    ///
    /// Only when the debhelper compat level is below 14 (or none is declared).
    /// Each installable named `gir<digits-and-dots>-...` whose Pre-Depends and
    /// Depends do not satisfy `${gir:Depends}` is a hit. Extra is the
    /// installable's name; the Location is `debian/control`.
    TypelibMissingGirDepends,
    code = "LN0916",
    tag = "typelib-missing-gir-depends",
    default_selected = true,
    experimental = false,
}

/// Compat level from which debhelper adds `${gir:Depends}` itself.
const MIN_DH_COMPAT_IMPLICIT_SUBSTVARS: u32 = 14;

static GIR_PACKAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^gir[\d.]+-").expect("valid regex"));

impl SourceRule for TypelibMissingGirDepends {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let compat = ctx.debhelper().and_then(|facts| facts.declared_level());
        if compat.is_some_and(|level| level >= MIN_DH_COMPAT_IMPLICIT_SUBSTVARS) {
            return;
        }
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let names: Vec<String> = control
            .installables()
            .iter()
            .filter(|installable| GIR_PACKAGE.is_match(installable.name()))
            .filter(|installable| {
                !installable
                    .fields()
                    .relations_and(&["Pre-Depends", "Depends"])
                    .satisfies("${gir:Depends}")
            })
            .map(|installable| installable.name().to_string())
            .collect();
        for name in names {
            ctx.diagnostic(Diagnostic::error(name).with_location(Location {
                path: "debian/control".into(),
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase, TestFile};

    use super::*;

    /// A packaging tree with one `gir1.2-*` installable. A debhelper compat
    /// level comes from `Build-Depends: debhelper-compat (= N)`, which is how a
    /// level of 14 or more is declared (a `debian/compat` file that high is
    /// rejected).
    fn control(compat: Option<u32>, depends: &str) -> TestCase {
        let build_depends = compat.map_or(String::new(), |level| {
            format!("Build-Depends: debhelper-compat (= {level})\n")
        });
        let mut files = vec![
            (
                "debian/control",
                TestFile::new(format!(
                    "Source: example\n{build_depends}\nPackage: gir1.2-example-1.0\n\
                     Architecture: any\nDepends: {depends}\nDescription: example\n extra\n"
                )),
            ),
            (
                "debian/rules",
                TestFile::new("#!/usr/bin/make -f\n\n%:\n\tdh $@\n"),
            ),
        ];
        if compat.is_none() {
            files.remove(1);
        }
        TestCase::source_tree(files)
    }

    #[test]
    fn test() {
        let pass = vec![
            control(Some(14), "libfoo"),
            control(Some(13), "${gir:Depends}, libfoo"),
            control(Some(13), "${misc:Depends}, ${gir:Depends}"),
            TestCase::source_tree([(
                "debian/control",
                TestFile::new(
                    "Source: example\n\nPackage: libexample\nArchitecture: any\n\
                     Depends: libfoo\nDescription: example\n extra\n",
                ),
            )]),
        ];
        let fail = vec![
            control(Some(13), "libfoo"),
            control(None, "libfoo"),
            control(Some(13), "${misc:Depends}"),
        ];
        SourceTreeTester::new(TypelibMissingGirDepends, pass, fail).test_and_snapshot();
    }
}
