use std::path::PathBuf;
use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::data;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

pub struct DeprecatedDebianControlField;

declare_rule! {
    /// A field of `debian/control` is one that is deprecated and should be
    /// dropped: `X-Python-Version`, `XB-Ruby-Versions`, `XS-Python-Version`
    /// or `XS-Ruby-Versions`. Reported on the source paragraph and on every
    /// installable paragraph. The extra is `(in section for source)` or
    /// `(in section for <installable>)`, then the field name, and the Location
    /// is `debian/control` at the line of the field.
    ///
    /// Field names are matched case-insensitively, as lintian's `resembles`
    /// does. Info severity and not selected by default.
    DeprecatedDebianControlField,
    code = "LN1518",
    tag = "deprecated-debian-control-field",
    default_selected = false,
    experimental = false,
}

/// lintian's `fields/deprecated-control-fields`.
static DEPRECATED: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    data::lines(include_str!(
        "../../../data/fields/deprecated-control-fields"
    ))
    .collect()
});

impl SourceRule for DeprecatedDebianControlField {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let mut hints: Vec<(String, Option<u32>)> = Vec::new();
        collect(control.source(), "(in section for source)", &mut hints);
        for installable in control.installables() {
            let section = format!("(in section for {})", installable.name());
            collect(installable.fields(), &section, &mut hints);
        }
        for (extra, line) in hints {
            ctx.diagnostic(
                Diagnostic::warn(extra)
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path: PathBuf::from("debian/control"),
                        line,
                        column: None,
                    }),
            );
        }
    }
}

/// The deprecated fields of one paragraph, as `(extra, line)`.
fn collect(paragraph: &ControlParagraph, section: &str, hints: &mut Vec<(String, Option<u32>)>) {
    for field in paragraph.names() {
        if DEPRECATED
            .iter()
            .any(|known| known.eq_ignore_ascii_case(field))
        {
            hints.push((format!("{section} {field}"), paragraph.position(field)));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase};

    use super::*;

    #[test]
    fn source() {
        let pass = vec![
            TestCase::from([(
                "debian/control",
                "Source: example\nMaintainer: A <a@example.com>\nXS-Autobuild: yes\n\nPackage: example\nArchitecture: all\nDescription: x\n y\n",
            )]),
            // A field that only resembles the name is not the name.
            TestCase::from([(
                "debian/control",
                "Source: example\nMaintainer: A <a@example.com>\nXS-Ruby-Version: 1\n\nPackage: example\nArchitecture: all\nDescription: x\n y\n",
            )]),
        ];
        let fail = vec![
            // In the source paragraph, in any case.
            TestCase::from([(
                "debian/control",
                "Source: example\nMaintainer: A <a@example.com>\nxs-ruby-versions: >= 2\n\nPackage: example\nArchitecture: all\nDescription: x\n y\n",
            )]),
            // In the source paragraph and in an installable.
            TestCase::from([(
                "debian/control",
                "Source: example\nMaintainer: A <a@example.com>\nX-Python-Version: 2.7\n\nPackage: example\nArchitecture: all\nXB-Ruby-Versions: 2\nDescription: x\n y\n",
            )]),
        ];
        SourceTreeTester::new(DeprecatedDebianControlField, pass, fail).test_and_snapshot();
    }
}
