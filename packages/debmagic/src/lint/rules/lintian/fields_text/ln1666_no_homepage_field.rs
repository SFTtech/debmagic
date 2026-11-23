use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};
use crate::lint::rules::lintian::fields_version::is_native;

pub struct NoHomepageField;

declare_rule! {
    /// A non-native source package has no `Homepage` field, in the source
    /// paragraph or in any binary package.
    ///
    /// Lintian's `fields/homepage` Check. A native package is skipped. Only
    /// reported when no binary package declares a Homepage either; that case
    /// is `homepage-in-binary-package` instead. Extra is empty and the Location
    /// is empty. Severity is pedantic and the Tag is not selected by default,
    /// as in lintian.
    NoHomepageField,
    code = "LN1666",
    tag = "no-homepage-field",
    default_selected = false,
    experimental = false,
}

impl SourceRule for NoHomepageField {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if is_native(ctx) {
            return;
        }
        let Some(control) = ctx.debian_control() else {
            return;
        };
        // A missing debian/control loads as an empty source paragraph: no
        // packaging, nothing to report.
        if control.source().names().next().is_none() {
            return;
        }
        if control.source().declares("Homepage")
            || control
                .installables()
                .iter()
                .any(|installable| installable.fields().declares("Homepage"))
        {
            return;
        }
        ctx.diagnostic(Diagnostic::warn("").with_severity(Severity::Pedantic));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn control(source_extra: &str, binary_extra: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: Jane <jane@example.com>\n\
                 Standards-Version: 4.7.2\n{source_extra}\n\
                 Package: example\nArchitecture: all\n{binary_extra}\
                 Description: example\n extra\n"
            )),
        )]
    }

    fn entry(version: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn tree(
        control: Vec<(&'static str, TestFile)>,
        version: &str,
    ) -> Vec<(&'static str, TestFile)> {
        let mut files = control;
        files.push(("debian/source/format", TestFile::new("3.0 (quilt)\n")));
        files.push(("debian/changelog", TestFile::new(entry(version))));
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree(control("Homepage: https://example.org/", ""), "1.0-1"),
            tree(control("", "Homepage: https://example.org/\n"), "1.0-1"),
            // A native package has no Homepage duty.
            [
                control("", ""),
                vec![
                    ("debian/source/format", TestFile::new("3.0 (native)\n")),
                    ("debian/changelog", TestFile::new(entry("1.0"))),
                ],
            ]
            .concat(),
        ];
        let fail = vec![tree(control("", ""), "1.0-1")];
        SourceTreeTester::new(NoHomepageField, pass, fail).test_and_snapshot();
    }
}
