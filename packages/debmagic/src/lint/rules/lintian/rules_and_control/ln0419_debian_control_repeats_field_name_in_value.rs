use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct DebianControlRepeatsFieldNameInValue;

declare_rule! {
    /// A `debian/control` field value starts by repeating the field name.
    ///
    /// The source paragraph and each installable are checked. The value is
    /// compared without regard to case: it starts with the field name, optional
    /// space, and a colon. Extra is `(in section for source)` or
    /// `(in section for <package>)`, then the field name. Location is
    /// `debian/control` at the field. A Source package and a Source tree share
    /// this check.
    DebianControlRepeatsFieldNameInValue,
    code = "LN0419",
    tag = "debian-control-repeats-field-name-in-value",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebianControlRepeatsFieldNameInValue {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let planned = {
            let Some(control) = ctx.debian_control() else {
                return;
            };
            let mut planned = Vec::new();
            let source = control.source();
            for field in repeated(source) {
                let line = source.position(&field);
                planned.push((None, field, line));
            }
            for installable in control.installables() {
                let fields = installable.fields();
                for field in repeated(fields) {
                    let line = fields.position(&field);
                    planned.push((Some(installable.name().to_string()), field, line));
                }
            }
            planned
        };
        if planned.is_empty() {
            return;
        }
        for (package, field, line) in planned {
            let section = match package {
                None => "(in section for source)".to_string(),
                Some(name) => format!("(in section for {name})"),
            };
            ctx.diagnostic(
                Diagnostic::warn(format!("{section} {field}")).with_location(Location {
                    path: PathBuf::from("debian/control"),
                    line,
                    column: None,
                }),
            );
        }
    }
}

fn repeated(paragraph: &ControlParagraph) -> Vec<String> {
    paragraph
        .names()
        .filter(|name| repeats_name(paragraph, name))
        .map(str::to_string)
        .collect()
}

fn repeats_name(paragraph: &ControlParagraph, name: &str) -> bool {
    let Some(value) = paragraph.get(name) else {
        return false;
    };
    let value = value.trim_start();
    let Some(rest) = value.get(name.len()..) else {
        return false;
    };
    value.is_char_boundary(name.len())
        && value[..name.len()].eq_ignore_ascii_case(name)
        && rest.trim_start().starts_with(':')
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn control(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/control", TestFile::new(body))]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/changelog", TestFile::new("not control\n"))],
            control(
                "Source: example\nMaintainer: Ada <ada@debian.org>\n\nPackage: example\nArchitecture: all\nDescription: A package\n",
            ),
            control("not a deb822 file\n: \n"),
        ];
        let fail = vec![
            control(
                "Source: example\nMaintainer: Maintainer: Ada <ada@debian.org>\n\nPackage: example\nArchitecture: all\nDescription: Description: broken\n",
            ),
            control(
                "Source: example\nSection: misc\n\nPackage: example-dbgsym\nDescription: Description: dbg-sym\n",
            ),
        ];
        SourceTreeTester::new(DebianControlRepeatsFieldNameInValue, pass, fail).test_and_snapshot();
    }
}
