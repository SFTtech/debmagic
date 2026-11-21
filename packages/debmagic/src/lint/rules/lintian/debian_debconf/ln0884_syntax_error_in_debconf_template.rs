use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

use super::files;

pub struct SyntaxErrorInDebconfTemplate;

declare_rule! {
    /// A debconf templates file does not parse as debconf Deb822.
    ///
    /// Binary packages: the installed `templates` control file. Source trees
    /// and packages: `debian/templates` and `debian/<installable>.templates`
    /// for each binary package in `debian/control`. The Extra is the parser's
    /// message in lintian's words, for example `Cannot parse line 3: <line>`,
    /// `Duplicate field <Name> (line N).`, `Continuation line not in
    /// paragraph (line N). Missing a dot on the previous line?`, or
    /// `Unexpected whitespace (line N). Missing a dot?`. The Location is the
    /// file with no line. A `#` line is a comment and a whitespace-only line
    /// is an error, as in debconf. Non-printable characters in a cannot-parse
    /// line become `_`. A file that is not valid UTF-8 is skipped.
    ///
    /// Difference: a PGP header line is not modelled. Lintian's reader accepts
    /// a signed paragraph and rejects a malformed signature with its own
    /// message; this Rule reports nothing for a file containing such a line.
    SyntaxErrorInDebconfTemplate,
    code = "LN0884",
    tag = "syntax-error-in-debconf-template",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SyntaxErrorInDebconfTemplate {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let hit = {
            let Some(tree) = ctx.control_files() else {
                return;
            };
            let Some(text) = files::read_utf8(tree, "templates") else {
                return;
            };
            first_error(&text.text).map(|message| (text.path, message))
        };
        if let Some((path, message)) = hit {
            ctx.diagnostic(Diagnostic::error(message).with_location(files::located(path, None)));
        }
    }
}

impl SourceRule for SyntaxErrorInDebconfTemplate {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let paths = catalog_paths(ctx);
        let hits: Vec<(PathBuf, String)> = {
            let Some(tree) = ctx.files() else {
                return;
            };
            paths
                .iter()
                .filter_map(|path| files::read_utf8(tree, path))
                .filter_map(|text| first_error(&text.text).map(|message| (text.path, message)))
                .collect()
        };
        for (path, message) in hits {
            ctx.diagnostic(Diagnostic::error(message).with_location(files::located(path, None)));
        }
    }
}

fn catalog_paths(ctx: &SourceContext<'_>) -> Vec<String> {
    let mut paths = vec!["debian/templates".to_string()];
    if let Some(control) = ctx.debian_control() {
        for installable in control.installables() {
            paths.push(format!("debian/{}.templates", installable.name()));
        }
    }
    paths
}

/// The first error lintian's Deb822 reader raises for a debconf template
/// file, as the Check prints it. `None` when the file parses.
///
/// Mirrors `visit_dpkg_paragraph_string` with the debconf flag only: a `#`
/// line is skipped, an empty line ends a paragraph, a whitespace-only line is
/// not empty, and a field name may repeat only after an empty-valued field
/// with the same name (the empty form does not check for duplicates).
fn first_error(text: &str) -> Option<String> {
    let mut open = false;
    let mut seen: Vec<String> = Vec::new();
    for (index, line) in text.split('\n').enumerate() {
        let position = index + 1;
        if line.starts_with('#') {
            continue;
        }
        if line.is_empty() {
            open = false;
            seen.clear();
            continue;
        }
        if line.starts_with("-----BEGIN PGP") || line.starts_with("-----END PGP") {
            return None;
        }
        if let Some((name, value)) = field(line) {
            if value.trim().is_empty() {
                open = true;
                if !seen.contains(&name) {
                    seen.push(name);
                }
                continue;
            }
            if seen.contains(&name) {
                return Some(format!("Duplicate field {name} (line {position})."));
            }
            open = true;
            seen.push(name);
            continue;
        }
        if is_continuation(line) {
            if !open {
                return Some(format!(
                    "Continuation line not in paragraph (line {position}). Missing a dot on the previous line?"
                ));
            }
            continue;
        }
        if line.chars().all(char::is_whitespace) {
            return Some(format!(
                "Unexpected whitespace (line {position}). Missing a dot?"
            ));
        }
        return Some(format!("Cannot parse line {position}: {}", printable(line)));
    }
    None
}

/// `^([^: \t]+):(.*)$`: the name runs to the first colon and has no space or
/// tab in it.
fn field(line: &str) -> Option<(String, String)> {
    let (name, value) = line.split_once(':')?;
    if name.is_empty() || name.contains([' ', '\t']) {
        return None;
    }
    Some((name.to_string(), value.to_string()))
}

/// `^([ \t].*\S.*)$`: indented, and some later character is not whitespace.
fn is_continuation(line: &str) -> bool {
    line.starts_with([' ', '\t']) && line.chars().any(|ch| !ch.is_whitespace())
}

/// Lintian replaces characters that are neither printable nor space.
fn printable(line: &str) -> String {
    line.chars()
        .map(|ch| {
            if ch.is_control() && !ch.is_whitespace() {
                '_'
            } else {
                ch
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{
        BinaryPackageCase, BinaryPackageTester, SourceTreeTester, TestCase, TestFile,
    };

    use super::*;

    fn templates(text: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("templates", TestFile::new(text))
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            templates("Template: debconf/x\nType: string\nDescription: Pick\n Extended\n"),
            templates("# comment\n\nTemplate: debconf/x\nType: string\n"),
            templates("Template: debconf/x\nDescription:\nType: string\n"),
            templates("Template: debconf/x\nType: string\n\nTemplate: debconf/y\nType: note\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
                "templates",
                TestFile::from_bytes(b"Template: a\n\xff\n".to_vec()),
            ),
        ];
        let fail = vec![
            templates("Template: debconf/x\nDescription whoops, I forgot a colon\n"),
            templates("Template: debconf/x\nType: string\nType: note\n"),
            templates("Template: debconf/x\n\n Continuation after a blank line\n"),
            templates("Template: debconf/x\n \nType: string\n"),
            templates("Template: debconf/x\n\x07bell\n"),
            templates("Template: debconf/x\nDescription: a\n\nDescription: b\nDescription: c\n"),
        ];
        BinaryPackageTester::new(SyntaxErrorInDebconfTemplate, pass, fail)
            .test_and_snapshot_with_suffix("binary");
    }

    const CONTROL: &str = "\
Source: example
Maintainer: Example <ex@example.com>

Package: example
Architecture: all
Description: example
 extra
";

    #[test]
    fn source() {
        let pass = vec![
            TestCase::from(Vec::<(&str, TestFile)>::new()),
            TestCase::from([("debian/templates", "Template: debconf/x\nType: string\n")]),
            TestCase::from([(
                "debian/templates",
                TestFile::from_bytes(vec![0xff, 0xfe, b'\n']),
            )]),
        ];
        let fail = vec![
            TestCase::from([("debian/templates", "Template: a\nno colon here\n")]),
            TestCase::from([
                ("debian/control", CONTROL),
                (
                    "debian/example.templates",
                    "Template: a\nType: note\nType: string\n",
                ),
            ]),
        ];
        SourceTreeTester::new(SyntaxErrorInDebconfTemplate, pass, fail).test_and_snapshot();
    }
}
