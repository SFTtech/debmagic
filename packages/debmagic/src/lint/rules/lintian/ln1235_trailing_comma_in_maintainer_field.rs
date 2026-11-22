use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::mailbox::parse_mailbox;

pub struct TrailingCommaInMaintainerField;

declare_rule! {
    /// The Maintainer field ends in a comma. Reported on the Subject, and
    /// again on `debian/control` when its source paragraph has the same
    /// problem.
    ///
    /// The Maintainer of the `.dsc` (Source package) is read; a Source tree has
    /// no `.dsc`, so its source paragraph is read with the trailing comma
    /// dropped, as dpkg-source writes it into the `.dsc`. Like lintian, it is
    /// only checked when it is a single mailbox with an address, where a
    /// trailing comma is allowed by the parser. Extra is the Maintainer value
    /// of the Subject. The `debian/control` report has Location `debian/control`
    /// without a line number.
    TrailingCommaInMaintainerField,
    code = "LN1235",
    tag = "trailing-comma-in-maintainer-field",
    default_selected = true,
    experimental = false,
}

/// A trailing comma after one mailbox still parses as that mailbox in lintian.
fn is_single_mailbox(value: &str) -> bool {
    let value = value.trim();
    let body = value.strip_suffix(',').unwrap_or(value);
    !body.contains(',') && parse_mailbox(body).is_some()
}

fn ends_with_comma(value: &str) -> bool {
    value.trim_end().ends_with(',')
}

impl SourceRule for TrailingCommaInMaintainerField {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let control = ctx.debian_control().map(|control| control.source().clone());
        let subject = match ctx.dsc() {
            Some(dsc) => dsc.fields().get("Maintainer").map(str::to_string),
            // No `.dsc` on a Source tree: use the Maintainer as dpkg-source
            // writes it into the `.dsc`, which drops the trailing comma.
            None => control.as_ref().and_then(|source| {
                source.get("Maintainer").map(|value| {
                    value
                        .trim_end()
                        .strip_suffix(',')
                        .unwrap_or(value)
                        .to_string()
                })
            }),
        };
        let Some(maintainer) = subject else {
            return;
        };
        if !is_single_mailbox(&maintainer) {
            return;
        }
        if ends_with_comma(&maintainer) {
            ctx.diagnostic(Diagnostic::error(maintainer.clone()));
        }
        let Some(source) = control else {
            return;
        };
        let Some(control_maintainer) = source.get("Maintainer") else {
            return;
        };
        if ends_with_comma(control_maintainer) {
            ctx.diagnostic(Diagnostic::error(maintainer).with_location(Location {
                path: PathBuf::from("debian/control"),
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestCase};

    use super::*;

    fn control(maintainer: &str) -> String {
        format!("Source: example\nMaintainer: {maintainer}\n")
    }

    fn dsc(maintainer: &str) -> Dsc {
        Dsc::parse(&format!(
            "Format: 3.0 (native)\nSource: example\nMaintainer: {maintainer}\n"
        ))
        .unwrap()
    }

    #[test]
    fn source_tree() {
        let pass = vec![
            TestCase::source_tree([("debian/control", control("Ex <ex@example.com>"))]),
            TestCase::source_tree([(
                "debian/control",
                control("Ex <ex@example.com>, Other <o@example.com>"),
            )]),
        ];
        let fail = vec![TestCase::source_tree([(
            "debian/control",
            control("Ex <ex@example.com>,"),
        )])];
        SourceTreeTester::new(TrailingCommaInMaintainerField, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![
            SourcePackageCase::from(vec![("debian/control", control("Ex <ex@example.com>"))])
                .dsc(dsc("Ex <ex@example.com>")),
            // Not a single mailbox: lintian does not check it.
            SourcePackageCase::from(vec![("debian/control", control("Ex <ex@example.com>,"))])
                .dsc(dsc("Ex <ex@example.com>, Other <o@example.com>,")),
        ];
        let fail = vec![
            SourcePackageCase::from(vec![("debian/control", control("Ex <ex@example.com>"))])
                .dsc(dsc("Ex <ex@example.com>,")),
            // Only the source paragraph of debian/control has the comma.
            SourcePackageCase::from(vec![("debian/control", control("Ex <ex@example.com>,"))])
                .dsc(dsc("Ex <ex@example.com>")),
        ];
        SourcePackageTester::new(TrailingCommaInMaintainerField, pass, fail).test_and_snapshot();
    }
}
