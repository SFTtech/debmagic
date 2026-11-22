use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct SubstvarSourceVersionIsDeprecated;

declare_rule! {
    /// A relation field of an installable uses the deprecated `${Source-Version}`.
    ///
    /// Checked for `Depends`, `Pre-Depends`, `Recommends`, `Suggests`,
    /// `Conflicts`, and `Replaces` of every installable in Debian control, in
    /// that order, where the field is declared. Reported whenever the field's
    /// text contains `${Source-Version}`, even when the substvar would not
    /// break anything. Extra is `<installable> <field>`. The Location is
    /// `debian/control`, with no line. Applies to the Source tree and Source
    /// package.
    SubstvarSourceVersionIsDeprecated,
    code = "LN1259",
    tag = "substvar-source-version-is-deprecated",
    default_selected = true,
    experimental = false,
}

const FIELDS: [&str; 6] = [
    "Depends",
    "Pre-Depends",
    "Recommends",
    "Suggests",
    "Conflicts",
    "Replaces",
];

const DEPRECATED: &str = "${Source-Version}";

impl SourceRule for SubstvarSourceVersionIsDeprecated {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let mut messages = Vec::new();
        for installable in control.installables() {
            let fields = installable.fields();
            for field in FIELDS {
                if !fields.declares(field) {
                    continue;
                }
                let Some(value) = fields.unfolded_value(field) else {
                    continue;
                };
                if value.contains(DEPRECATED) {
                    messages.push(format!("{} {field}", installable.name()));
                }
            }
        }
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message).with_location(Location {
                path: PathBuf::from("debian/control"),
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(installable: &str) -> String {
        format!(
            "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
Build-Depends: debhelper-compat (= 13)\n\
\n\
Package: example\n\
Architecture: all\n\
{installable}\
Description: example\n extra\n"
        )
    }

    fn package(installable: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/control", TestFile::new(control(installable)))]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no control"))],
            package(""),
            package("Depends: foo (= ${binary:Version})\n"),
            package("Depends: foo (= ${source:Version})\n"),
            // Not a relation field.
            package("Provides: foo (= ${Source-Version})\n"),
            package("Description: uses ${Source-Version}\n"),
        ];
        let fail = vec![
            package("Depends: foo (= ${Source-Version})\n"),
            package("Recommends: foo (= ${Source-Version})\n"),
            package("Conflicts: foo (<< ${Source-Version})\n"),
            // Reported even without a versioned relation to break.
            package("Suggests: ${Source-Version}\n"),
        ];
        SourceTreeTester::new(SubstvarSourceVersionIsDeprecated, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package("Depends: foo\n"))];
        let fail = vec![SourcePackageCase::from(package(
            "Depends: foo (= ${Source-Version})\n",
        ))];
        SourcePackageTester::new(SubstvarSourceVersionIsDeprecated, pass, fail).test_and_snapshot();
    }
}
