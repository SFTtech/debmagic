use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct TabInLicenseText;

declare_rule! {
    /// A `License` field of a machine-readable copyright holds a tab.
    ///
    /// Reads `debian/copyright` and `debian/<installable>.copyright` when they
    /// are machine-readable and parse. Every paragraph with a `License` field is
    /// checked, the header included, on the field as stored (the short name and
    /// the license text together). Extra is empty; the Location is the line
    /// where the `License` field starts.
    TabInLicenseText,
    code = "LN1022",
    tag = "tab-in-license-text",
    default_selected = true,
    experimental = false,
}

impl SourceRule for TabInLicenseText {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for (path, copyright) in ctx.debian_copyrights() {
            let Some(paragraphs) = copyright.paragraphs() else {
                continue;
            };
            for paragraph in paragraphs {
                if !paragraph.declares("License")
                    || !paragraph.untrimmed_value("License").contains('\t')
                {
                    continue;
                }
                ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                    path: path.clone(),
                    line: paragraph.position("License"),
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

    const FORMAT: &str =
        "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n";

    fn copyright(text: &str) -> TestFile {
        TestFile::new(format!("{FORMAT}{text}"))
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![(
                "debian/copyright",
                copyright("\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n Permission\n"),
            )],
            // A tab in another field is not the license text.
            vec![(
                "debian/copyright",
                copyright("\nFiles: *\nCopyright:\t2024 Someone\nLicense: MIT\n"),
            )],
            // Not parsed: an outdated draft is never read.
            vec![(
                "debian/copyright",
                TestFile::new(
                    "Format: http://svn.debian.org/wsvn/dep/web/deps/dep5.mdwn?op=file&rev=147\n\nLicense: MIT\n\tbody\n",
                ),
            )],
        ];
        let fail = vec![
            // Tab in the text of a `Files` paragraph.
            vec![(
                "debian/copyright",
                copyright("\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n\tPermission\n"),
            )],
            // Tab in a stand-alone license paragraph: the line is the field's.
            vec![(
                "debian/copyright",
                copyright(
                    "\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n\nLicense: GPL-2\n\tbody\n",
                ),
            )],
            // Tab on the first line of a license, after the short name.
            vec![(
                "debian/copyright",
                copyright("\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\tx\n Body\n"),
            )],
        ];
        SourceTreeTester::new(TabInLicenseText, pass, fail).test_and_snapshot();
    }
}
