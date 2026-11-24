use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct SpaceInStdShortnameInDep5Copyright;

declare_rule! {
    /// A DEP-5 license short name contains a space and is not a `with` exception.
    ///
    /// Reads `debian/copyright` and `debian/<package>.copyright` when they are
    /// copyright-format 1.0. The first line of `License` is split on `and` /
    /// `or`. Extra is the short name. The Location is the file and the
    /// `License` line.
    SpaceInStdShortnameInDep5Copyright,
    code = "LN0121",
    tag = "space-in-std-shortname-in-dep5-copyright",
    default_selected = true,
    experimental = false,
}

static AND_OR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+(?:and|or)\s+").expect("valid regex"));
static WITH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^ ]+\s+with\s+").expect("valid regex"));

impl SourceRule for SpaceInStdShortnameInDep5Copyright {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let hits: Vec<(std::path::PathBuf, u32, String)> = ctx
            .debian_copyrights()
            .into_iter()
            .flat_map(|(path, copyright)| {
                let paragraphs = copyright.paragraphs().unwrap_or(&[]).to_vec();
                paragraphs
                    .into_iter()
                    .filter(|paragraph| paragraph.declares("License"))
                    .flat_map(|paragraph| {
                        let line = paragraph.position("License").unwrap_or(1);
                        let path = path.clone();
                        names(paragraph.untrimmed_value("License"))
                            .into_iter()
                            .map(move |name| (path.clone(), line, name))
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        for (path, line, name) in hits {
            ctx.diagnostic(Diagnostic::warn(name).with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

fn names(value: &str) -> Vec<String> {
    let first = value.split('\n').next().unwrap_or("");
    let identifier = first
        .replace([',', '(', ')'], " ")
        .trim()
        .to_ascii_lowercase();
    AND_OR
        .split(&identifier)
        .filter(|name| !name.is_empty() && name.contains(' ') && !WITH.is_match(name))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn copyright(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/copyright",
            TestFile::new(format!(
                "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n\
                 Upstream-Name: example\n\n{body}"
            )),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            copyright("Files: *\nCopyright: 2020 Jane\nLicense: GPL-2\n"),
            copyright("Files: *\nCopyright: 2020 Jane\nLicense: GPL-2 with exception\n"),
        ];
        let fail = vec![
            copyright("Files: *\nCopyright: 2020 Jane\nLicense: some space license\n"),
            copyright("Files: *\nCopyright: 2020 Jane\nLicense: this | that\n"),
        ];
        SourceTreeTester::new(SpaceInStdShortnameInDep5Copyright, pass, fail).test_and_snapshot();
    }
}
