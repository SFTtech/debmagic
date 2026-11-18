use std::collections::{HashMap, HashSet};

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::dep5::{license_identifier, license_names, license_text};

pub struct MissingLicenseParagraphInDep5Copyright;

declare_rule! {
    /// A `Files` paragraph names a license that has no stand-alone paragraph.
    ///
    /// Reads `debian/copyright` and `debian/<package>.copyright` when they are
    /// machine-readable. A `Files` paragraph whose `License` has no body
    /// requires each short name. A paragraph with a license body and no
    /// `Files` field supplies the whole first line. Extra is the missing short
    /// name. The Location is the `License` line of the last requiring paragraph.
    MissingLicenseParagraphInDep5Copyright,
    code = "LN0444",
    tag = "missing-license-paragraph-in-dep5-copyright",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MissingLicenseParagraphInDep5Copyright {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let copyrights = ctx.debian_copyrights();
        for (path, copyright) in copyrights {
            let Some(paragraphs) = copyright.paragraphs() else {
                continue;
            };
            let mut found = HashSet::new();
            let mut required: HashMap<String, u32> = HashMap::new();
            let mut order = Vec::new();
            for paragraph in paragraphs {
                if !paragraph.declares("License") {
                    continue;
                }
                let raw = paragraph.untrimmed_value("License");
                let identifier = license_identifier(raw);
                let text = license_text(raw);
                let line = paragraph
                    .position("License")
                    .unwrap_or(paragraph.start_line());
                if !identifier.is_empty() && !text.is_empty() && !paragraph.declares("Files") {
                    found.insert(identifier.clone());
                }
                if paragraph.declares("Files") && text.is_empty() {
                    for name in license_names(&identifier) {
                        if !required.contains_key(&name) {
                            order.push(name.clone());
                        }
                        required.insert(name, line);
                    }
                }
            }
            for name in order {
                if found.contains(&name) {
                    continue;
                }
                let Some(line) = required.get(&name).copied() else {
                    continue;
                };
                ctx.diagnostic(Diagnostic::warn(name).with_location(Location {
                    path: path.clone(),
                    line: Some(line),
                    column: None,
                }));
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
            tree(
                "Files: *\nCopyright: 2020 Jane\nLicense: GPL-2\n\n\
                 License: GPL-2\n Full text.\n",
            ),
            tree("Files: *\nCopyright: 2020 Jane\nLicense: Expat\n Permission is granted.\n"),
            vec![("debian/copyright", TestFile::new("Copyright 2020 Jane\n"))],
        ];
        let fail = vec![
            tree("Files: d\nLicense: GPL-3\n\nFiles: 5\nLicense: MIT\nCopyright: 2020 Jane\n"),
            tree("Files: *\nCopyright: 2020 Jane\nLicense: GPL-2 or MIT\n"),
        ];
        SourceTreeTester::new(MissingLicenseParagraphInDep5Copyright, pass, fail)
            .test_and_snapshot();
    }
}
