use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_copyright::CopyrightParagraph;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct MissingFieldInDep5Copyright;

declare_rule! {
    /// A DEP-5 `Files` paragraph is missing `License` or `Copyright`, or one of
    /// those fields (or `Files`) is declared empty.
    ///
    /// Reads `debian/copyright` and `debian/<package>.copyright` when they are
    /// machine-readable. The header paragraph is not a `Files` paragraph. Extra
    /// is `License`, `Copyright`, or `(empty field)` plus the field name, in
    /// the Check's order. The Location is the field line, or the paragraph's
    /// first line when the field is absent.
    MissingFieldInDep5Copyright,
    code = "LN0356",
    tag = "missing-field-in-dep5-copyright",
    default_selected = true,
    experimental = false,
}

impl SourceRule for MissingFieldInDep5Copyright {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let copyrights = ctx.debian_copyrights();
        for (path, copyright) in copyrights {
            let Some(paragraphs) = copyright.paragraphs() else {
                continue;
            };
            let Some((_header, followers)) = paragraphs.split_first() else {
                continue;
            };
            for paragraph in followers
                .iter()
                .filter(|paragraph| paragraph.declares("Files"))
            {
                for (line, extra) in missing_fields(paragraph) {
                    ctx.diagnostic(Diagnostic::warn(extra).with_location(Location {
                        path: path.clone(),
                        line: Some(line),
                        column: None,
                    }));
                }
            }
        }
    }
}

fn missing_fields(paragraph: &CopyrightParagraph) -> Vec<(u32, String)> {
    let start = paragraph.start_line();
    let mut found = Vec::new();
    if paragraph.value("Files").is_empty() {
        found.push((
            paragraph.position("Files").unwrap_or(start),
            "(empty field) Files".to_string(),
        ));
    }
    if !paragraph.declares("License") {
        found.push((start, "License".to_string()));
    }
    if !paragraph.declares("Copyright") {
        found.push((start, "Copyright".to_string()));
    }
    if paragraph.declares("Copyright") && paragraph.value("Copyright").is_empty() {
        found.push((
            paragraph.position("Copyright").unwrap_or(start),
            "(empty field) Copyright".to_string(),
        ));
    }
    found
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

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
            tree("Files: *\nCopyright: 2020 Jane\nLicense: Expat\n"),
            // The header is not a Files paragraph, even when those fields are empty.
            vec![(
                "debian/copyright",
                TestFile::new(format!(
                    "{FORMAT}Files:\nCopyright:\nLicense: Expat\n\n\
                     Files: *\nCopyright: 2020 Jane\nLicense: Expat\n"
                )),
            )],
            vec![("debian/copyright", TestFile::new("Copyright 2020 Jane\n"))],
            vec![(
                "debian/copyright",
                TestFile::new(format!("{FORMAT}Files: a\nFiles: b\n")),
            )],
            Vec::new(),
        ];
        let fail = vec![
            tree("Files:\nCopyright: 2020 Jane\nLicense: Expat\n text\n"),
            tree("Files: *\nCopyright:\nLicense: Expat\n"),
            tree("Files: c\nCopyright: 2020 Jane\n"),
            tree("Files: d\nLicense: Expat\n"),
            tree("Files:\nCopyright:\n"),
        ];
        SourceTreeTester::new(MissingFieldInDep5Copyright, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(tree(
            "Files: *\nCopyright: 2020 Jane\nLicense: Expat\n",
        ))];
        let fail = vec![SourcePackageCase::from(tree(
            "Files: c\nCopyright: 2020 Jane\n",
        ))];
        SourcePackageTester::new(MissingFieldInDep5Copyright, pass, fail).test_and_snapshot();
    }
}
