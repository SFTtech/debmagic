use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct MultilineArchitectureField;

declare_rule! {
    /// The `Architecture` field of an installable spans several lines.
    ///
    /// Checks the `Architecture` field of every installable paragraph of
    /// `debian/control` (Source tree, or Source package Patched files). Like
    /// lintian's `value =~ /\n./`, a field is multiline when its trimmed value
    /// has a line break followed by text. The source paragraph is not checked.
    /// Extra is `Architecture (in section for <installable>)`. The Location is
    /// `debian/control` at the line the field starts on, found by scanning the
    /// text (no line is given when the file cannot be read).
    MultilineArchitectureField,
    code = "LN1016",
    tag = "multiline-architecture-field",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MultilineArchitectureField {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(control) = ctx.debian_control() else {
            return;
        };
        let multiline: Vec<String> = control
            .installables()
            .iter()
            .filter(|installable| {
                installable
                    .fields()
                    .untrimmed_value("Architecture")
                    .is_some_and(is_multiline)
            })
            .map(|installable| installable.name().to_string())
            .collect();
        if multiline.is_empty() {
            return;
        }
        let text = control_text(ctx);
        for name in multiline {
            let line = text
                .as_deref()
                .and_then(|text| field_line(text, &name, "Architecture"));
            ctx.diagnostic(
                Diagnostic::error(format!("Architecture (in section for {name})")).with_location(
                    Location {
                        path: PathBuf::from("debian/control"),
                        line,
                        column: None,
                    },
                ),
            );
        }
    }
}

/// lintian's `value =~ /\n./` on the trimmed value.
fn is_multiline(untrimmed: &str) -> bool {
    untrimmed
        .trim()
        .split('\n')
        .skip(1)
        .any(|line| !line.is_empty())
}

fn control_text(ctx: &mut SourceContext<'_>) -> Option<String> {
    let files = ctx.files()?;
    let entry = files.get("debian/control")?;
    String::from_utf8(files.read(entry).ok()?).ok()
}

/// 1-based line where `field` starts in the paragraph whose `Package` is `package`.
fn field_line(text: &str, package: &str, field: &str) -> Option<u32> {
    let mut paragraphs: Vec<Vec<(u32, &str)>> = vec![Vec::new()];
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            paragraphs.push(Vec::new());
        } else if let Some(paragraph) = paragraphs.last_mut() {
            paragraph.push((u32::try_from(index).unwrap_or(u32::MAX) + 1, line));
        }
    }
    let paragraph = paragraphs.iter().find(|paragraph| {
        paragraph
            .iter()
            .any(|(_, line)| field_value(line, "Package") == Some(package))
    })?;
    paragraph
        .iter()
        .find(|(_, line)| field_value(line, field).is_some())
        .map(|(number, _)| *number)
}

/// The value of a `name: value` line; `None` for continuation and comment lines.
fn field_value<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    if line.starts_with([' ', '\t', '#']) {
        return None;
    }
    let (key, value) = line.split_once(':')?;
    key.eq_ignore_ascii_case(name).then(|| value.trim())
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const SOURCE: &str = "Source: example\nMaintainer: Example <ex@example.com>\n\n";

    fn control(text: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/control", TestFile::new(format!("{SOURCE}{text}")))]
    }

    fn installable(architecture: &str) -> String {
        format!("Package: example\nArchitecture: {architecture}\nDescription: example\n extra\n")
    }

    #[test]
    fn source() {
        let multiline = "Architecture: any-amd64 any-arm64\n              any-mips64\n";
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            control(&installable("any amd64")),
            control(&installable("all")),
            control("Package: example\nArchitecture:\n"),
            control("Package: example\nDescription: no architecture\n extra\n"),
            // A multiline value in the source paragraph is not checked.
            vec![(
                "debian/control",
                TestFile::new(
                    "Source: example\nArchitecture: any\n  all\n\nPackage: example\n\
                     Architecture: all\nDescription: example\n extra\n",
                ),
            )],
        ];
        let fail = vec![
            control(&format!(
                "Package: example\n{multiline}Description: example\n extra\n"
            )),
            control("Package: example\nArchitecture: any\n all\nDescription: example\n extra\n"),
            control(&format!(
                "{}\nPackage: other\n{multiline}Description: other\n extra\n",
                installable("all")
            )),
        ];
        SourceTreeTester::new(MultilineArchitectureField, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let dsc = || Dsc::parse("Source: example\n").expect("Dsc");
        let pass = vec![SourcePackageCase::from(control(&installable("all"))).dsc(dsc())];
        let fail = vec![
            SourcePackageCase::from(control(
                "Package: example\nArchitecture: any\n all\nDescription: example\n extra\n",
            ))
            .dsc(dsc()),
        ];
        SourcePackageTester::new(MultilineArchitectureField, pass, fail).test_and_snapshot();
    }
}
