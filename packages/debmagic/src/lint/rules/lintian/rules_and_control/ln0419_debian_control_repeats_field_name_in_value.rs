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
            for field in repeated(control.source()) {
                planned.push((None, field));
            }
            for installable in control.installables() {
                for field in repeated(installable.fields()) {
                    planned.push((Some(installable.name().to_string()), field));
                }
            }
            planned
        };
        if planned.is_empty() {
            return;
        }
        let text = ctx
            .files()
            .and_then(|files| {
                let entry = files.get("debian/control")?;
                let bytes = files.read(entry).ok()?;
                String::from_utf8(bytes).ok()
            })
            .unwrap_or_default();
        let paragraphs = raw_paragraphs(&text);
        for (package, field) in planned {
            let paragraph = match &package {
                None => paragraphs.first(),
                Some(name) => paragraphs.iter().find(|paragraph| {
                    field_value(paragraph, "Package").as_deref() == Some(name.as_str())
                }),
            };
            let line = paragraph.and_then(|paragraph| field_line(paragraph, &field));
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

fn raw_paragraphs(text: &str) -> Vec<Vec<(u32, String)>> {
    let mut paragraphs = vec![Vec::new()];
    for (index, line) in text.split('\n').enumerate() {
        let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
        if line.trim().is_empty() {
            if paragraphs
                .last()
                .is_some_and(|paragraph| !paragraph.is_empty())
            {
                paragraphs.push(Vec::new());
            }
            continue;
        }
        paragraphs
            .last_mut()
            .expect("paragraph list starts non-empty")
            .push((number, line.to_string()));
    }
    paragraphs.retain(|paragraph| !paragraph.is_empty());
    paragraphs
}

fn field_line(paragraph: &[(u32, String)], field: &str) -> Option<u32> {
    paragraph.iter().find_map(|(number, line)| {
        if line.starts_with([' ', '\t']) {
            return None;
        }
        let name = line.split_once(':')?.0.trim();
        name.eq_ignore_ascii_case(field).then_some(*number)
    })
}

fn field_value(paragraph: &[(u32, String)], field: &str) -> Option<String> {
    paragraph.iter().find_map(|(_, line)| {
        if line.starts_with([' ', '\t']) {
            return None;
        }
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case(field)
            .then(|| value.trim().to_string())
    })
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
