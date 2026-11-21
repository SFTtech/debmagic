use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct PypiHomepage;

declare_rule! {
    /// The Homepage field points at the Python Package Index rather than at the
    /// upstream project (Bug#981932).
    ///
    /// The Homepage of the Dsc on a Source package, or of the source paragraph
    /// of `debian/control` on a Source tree, is reported when it starts with
    /// `http` or `https`, an optional `www.`, then `pypi.org/`,
    /// `pypi.python.org/` or `pypi:.python.org/` (case-insensitive). The value is
    /// unfolded and trimmed; it is the extra. Location is empty.
    PypiHomepage,
    code = "LN1117",
    tag = "pypi-homepage",
    default_selected = true,
    experimental = false,
}

static PYPI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^https?://(?:www\.)?pypi(?::?\.python)?\.org/").expect("valid regex")
});

impl SourceRule for PypiHomepage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let homepage = match ctx.dsc() {
            Some(dsc) => dsc.fields().unfolded_value("Homepage"),
            None => ctx
                .debian_control()
                .and_then(|control| control.source().unfolded_value("Homepage")),
        };
        let Some(homepage) = homepage else {
            return;
        };
        if PYPI.is_match(&homepage) {
            ctx.diagnostic(Diagnostic::warn(homepage));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(homepage: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nHomepage: {homepage}\n\nPackage: example\nArchitecture: all\n\
                 Description: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn source() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            control("https://example.org/"),
            control("https://pypi.example.org/"),
            control("https://example.org/pypi.org/"),
            control("https://pypi.org.example.com/"),
            control("ftp://pypi.org/project/x/"),
            control("https://pypi.org"),
        ];
        let fail = vec![
            control("https://pypi.org/project/some-name/"),
            control("http://pypi.org/project/some-name/"),
            control("https://www.pypi.org/project/some-name/"),
            control("https://pypi.python.org/pypi/some-name"),
            control("https://PyPI.org/project/some-name/"),
            control("https://pypi:.python.org/project/some-name/"),
        ];
        SourceTreeTester::new(PypiHomepage, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let dsc = |homepage: &str| {
            Dsc::parse(&format!("Source: example\nHomepage: {homepage}\n"))
                .expect("valid dsc in test")
        };
        let pass = vec![
            SourcePackageCase::from(control("https://example.org/"))
                .dsc(dsc("https://example.org/")),
        ];
        let fail = vec![
            SourcePackageCase::from(control("https://example.org/"))
                .dsc(dsc("https://pypi.org/project/some-name/")),
        ];
        SourcePackageTester::new(PypiHomepage, pass, fail).test_and_snapshot();
    }
}
