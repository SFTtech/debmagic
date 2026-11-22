use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};
use crate::lint::rules::lintian::fields_version::is_native;

pub struct HomepageInBinaryPackage;

declare_rule! {
    /// A non-native source package has a binary package with a `Homepage`
    /// field, but the source paragraph has none.
    ///
    /// Lintian's `fields/homepage` Check. Moving the field to the source
    /// paragraph lets every binary package inherit it, and lets source-based
    /// services such as the PTS link to the upstream project. Native sources
    /// are skipped, as lintian does. Extra is the name of each binary package
    /// that declares a Homepage, in `debian/control` order. Location is empty.
    HomepageInBinaryPackage,
    code = "LN1549",
    tag = "homepage-in-binary-package",
    default_selected = false,
    experimental = false,
}

impl SourceRule for HomepageInBinaryPackage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if is_native(ctx) {
            return;
        }
        let Some(control) = ctx.debian_control() else {
            return;
        };
        if control.source().declares("Homepage") {
            return;
        }
        let names: Vec<String> = control
            .installables()
            .iter()
            .filter(|installable| installable.fields().declares("Homepage"))
            .map(|installable| installable.name().to_string())
            .collect();
        for name in names {
            ctx.diagnostic(Diagnostic::warn(name).with_severity(Severity::Info));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const SOURCE: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n";

    const BINARY: &str = "Package: other\n\
         Architecture: all\n\
         Description: example\n extra\n";

    const BINARY_WITH_HOMEPAGE: &str = "Package: example\n\
         Architecture: all\n\
         Homepage: https://example.org/\n\
         Description: example\n extra\n";

    fn entry(version: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn tree(
        source: Option<&str>,
        binaries: &[&str],
        version: &str,
        format: &str,
    ) -> Vec<(&'static str, TestFile)> {
        let mut control = source.unwrap_or(SOURCE).to_string();
        for binary in binaries {
            control.push('\n');
            control.push_str(binary);
        }
        vec![
            ("debian/control", TestFile::new(control)),
            ("debian/changelog", TestFile::new(entry(version))),
            ("debian/source/format", TestFile::new(format)),
        ]
    }

    #[test]
    fn source_tree() {
        let homepage_source = format!("{SOURCE}Homepage: https://example.org/\n");
        let pass = vec![
            tree(None, &[BINARY], "1.0-1", "3.0 (quilt)"),
            tree(
                Some(homepage_source.as_str()),
                &[BINARY_WITH_HOMEPAGE],
                "1.0-1",
                "3.0 (quilt)",
            ),
            tree(None, &[BINARY_WITH_HOMEPAGE], "1.0", "3.0 (native)"),
        ];
        let fail = vec![
            tree(None, &[BINARY_WITH_HOMEPAGE], "1.0-1", "3.0 (quilt)"),
            tree(
                None,
                &[BINARY, BINARY_WITH_HOMEPAGE],
                "1.0-1",
                "3.0 (quilt)",
            ),
        ];
        SourceTreeTester::new(HomepageInBinaryPackage, pass, fail).test_and_snapshot();
    }
}
