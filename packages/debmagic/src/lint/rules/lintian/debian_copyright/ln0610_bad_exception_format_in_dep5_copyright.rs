use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dep5::{license_identifier, license_names};

pub struct BadExceptionFormatInDep5Copyright;

declare_rule! {
    /// A DEP-5 license short name uses `with` for something other than an exception.
    ///
    /// Reads `debian/copyright` and `debian/<package>.copyright` when they are
    /// machine-readable. The first line of `License` is lowercased and split on
    /// `and` / `or`. A short name with a space and `with` is reported unless the
    /// text after `with` has a word followed by `exception`. Extra is the short
    /// name. The Location is the `License` line.
    BadExceptionFormatInDep5Copyright,
    code = "LN0610",
    tag = "bad-exception-format-in-dep5-copyright",
    default_selected = true,
    experimental = false,
}

static WITH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^ ]+\s+with\s+(.*)").expect("valid regex"));
static EXCEPTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^ ]+\s+exception").expect("valid regex"));

/// A short name that has a space, uses `with`, and does not end in
/// `<word> exception`.
fn bad_exception(name: &str) -> bool {
    if !name.contains(char::is_whitespace) {
        return false;
    }
    WITH.captures(name)
        .is_some_and(|captures| !EXCEPTION.is_match(&captures[1]))
}

impl SourceRule for BadExceptionFormatInDep5Copyright {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let copyrights = ctx.debian_copyrights();
        for (path, copyright) in copyrights {
            let Some(paragraphs) = copyright.paragraphs() else {
                continue;
            };
            for paragraph in paragraphs
                .iter()
                .filter(|paragraph| paragraph.declares("License"))
            {
                let line = paragraph
                    .position("License")
                    .unwrap_or(paragraph.start_line());
                let identifier = license_identifier(paragraph.untrimmed_value("License"));
                for name in license_names(&identifier) {
                    if !bad_exception(&name) {
                        continue;
                    }
                    ctx.diagnostic(Diagnostic::warn(name).with_location(Location {
                        path: path.clone(),
                        line: Some(line),
                        column: None,
                    }));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const FORMAT: &str = "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n\
         Upstream-Name: example\n";

    fn tree(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/copyright",
            TestFile::new(format!("{FORMAT}\n{body}")),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            tree("Files: *\nCopyright: 2020 Jane\nLicense: GPL-3+ with OpenSSL exception\n"),
            tree("Files: *\nCopyright: 2020 Jane\nLicense: GPL-3\n"),
            tree("Files: *\nCopyright: 2020 Jane\nLicense: some space license\n"),
            tree("Files: *\nCopyright: 2020 Jane\nLicense: GPL-3+ with OpenSSL exception or MIT\n"),
        ];
        let fail = vec![
            tree("Files: *\nCopyright: 2020 Jane\nLicense: GPL-3 with font\n"),
            tree("Files: *\nCopyright: 2020 Jane\nLicense: GPL-3+ with exception\n"),
            tree("Files: *\nCopyright: 2020 Jane\nLicense: GPL-3 with font and MIT\n"),
            tree(
                "Files: a\nCopyright: 2020 Jane\nLicense: LGPL-2 with something else\n\nFiles: b\nCopyright: 2020 Jane\nLicense: GPL-3 with font\n",
            ),
        ];
        SourceTreeTester::new(BadExceptionFormatInDep5Copyright, pass, fail).test_and_snapshot();
    }
}
