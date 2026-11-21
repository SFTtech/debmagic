use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct RelativeConffile;

declare_rule! {
    /// A `conffiles` entry has no leading `/`.
    ///
    /// Each line is right-trimmed, skipped when empty, and split on whitespace.
    /// The last word is the path; a leading `/` makes it absolute. Earlier
    /// words are instructions. Extra is the path as written. Location is
    /// `conffiles` at that line. A non-UTF-8 member is skipped. Udebs are
    /// checked too.
    RelativeConffile,
    code = "LN1012",
    tag = "relative-conffile",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for RelativeConffile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(text) = conffiles_text(ctx) else {
            return;
        };
        for entry in parse(&text) {
            if entry.relative {
                ctx.diagnostic(Diagnostic::error(entry.path).with_location(Location {
                    path: PathBuf::from("conffiles"),
                    line: Some(entry.line),
                    column: None,
                }));
            }
        }
    }
}

/// One `conffiles` line, as lintian's `Lintian::Conffiles` parses it.
pub(super) struct ConffileLine {
    /// 1-based line number, counting blank lines.
    pub(super) line: u32,
    /// The last word, with one leading `/` removed.
    pub(super) path: String,
    /// True when the last word had no leading `/`.
    pub(super) relative: bool,
    /// The words before the path, in file order.
    pub(super) instructions: Vec<String>,
}

/// The `conffiles` member as UTF-8 text, or `None` when it is missing or not
/// UTF-8.
pub(super) fn conffiles_text(ctx: &mut BinaryPackageContext<'_>) -> Option<String> {
    let control = ctx.control_files()?;
    let entry = control.get("conffiles")?;
    let bytes = control.read(entry).ok()?;
    String::from_utf8(bytes).ok()
}

/// Parse the non-blank lines of a `conffiles` member.
pub(super) fn parse(text: &str) -> Vec<ConffileLine> {
    text.split('\n')
        .enumerate()
        .filter_map(|(index, raw)| {
            let line = raw.trim_end();
            if line.is_empty() {
                return None;
            }
            let mut words: Vec<&str> = line.split_whitespace().collect();
            let last = words.pop()?;
            let (relative, path) = match last.strip_prefix('/') {
                Some(path) => (false, path),
                None => (true, last),
            };
            Some(ConffileLine {
                line: u32::try_from(index + 1).unwrap_or(u32::MAX),
                path: path.to_string(),
                relative,
                instructions: words.into_iter().map(str::to_string).collect(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn conffiles(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("conffiles", TestFile::new(body))
    }

    #[test]
    fn parse_keeps_line_numbers_and_absoluteness() {
        let lines: Vec<_> = parse("/etc/foo\n\n remove-on-upgrade  etc/bar \n  \nbaz")
            .into_iter()
            .map(|entry| (entry.line, entry.path, entry.relative, entry.instructions))
            .collect();
        assert_eq!(
            lines,
            vec![
                (1, "etc/foo".to_string(), false, vec![]),
                (
                    3,
                    "etc/bar".to_string(),
                    true,
                    vec!["remove-on-upgrade".to_string()]
                ),
                (5, "baz".to_string(), true, vec![]),
            ]
        );
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            conffiles(""),
            conffiles("/etc/foo\n"),
            conffiles("/etc/foo\n\n/etc/bar\n"),
            conffiles("remove-on-upgrade /etc/gone\n"),
            conffiles("//etc/foo\n"),
        ];
        let fail = vec![
            conffiles("etc/foo\n"),
            conffiles("/etc/foo\n\n  \netc/bar\n"),
            conffiles("remove-on-upgrade etc/gone\n"),
            conffiles("chicken-soup etc/foo\n").udeb(),
            conffiles("etc/foo  \n"),
        ];
        BinaryPackageTester::new(RelativeConffile, pass, fail).test_and_snapshot();
    }
}
