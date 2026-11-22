use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};

use super::ln0396_format_3_0_but_debian_changes_patch::{is_quilt_format, source_format};
use super::ln0706_patch_file_present_but_not_mentioned_in_series::BUILD_DEPENDS_ALL;
use super::ln1325_quilt_patch_missing_description::readable;

pub struct QuiltPatchUsingTemplateDescription;

declare_rule! {
    /// A quilt patch still carries the template description line.
    ///
    /// Runs when Format is `3.x (quilt)` or Build-Depends-All satisfies
    /// `quilt`, and `debian/patches/series` is readable. Each patch the series
    /// names that is a file is scanned up to the first line starting with
    /// `---`. When a line contains `TODO: Put a short summary on the line above
    /// and replace this paragraph`, the patch gets a Diagnostic with the patch
    /// as Location, once. Info, not selected by default. Patches that are not
    /// files are the other Tag's business.
    QuiltPatchUsingTemplateDescription,
    code = "LN1530",
    tag = "quilt-patch-using-template-description",
    default_selected = false,
    experimental = false,
}

const PATCH_DESC_TEMPLATE: &str =
    "TODO: Put a short summary on the line above and replace this paragraph";

impl SourceRule for QuiltPatchUsingTemplateDescription {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let format = source_format(ctx);
        let quilt_format = is_quilt_format(&format);
        let uses_quilt = quilt_format
            || ctx.debian_control().is_some_and(|control| {
                control
                    .source()
                    .relations_and(BUILD_DEPENDS_ALL)
                    .satisfies("quilt")
            });
        if !uses_quilt {
            return;
        }
        let Some(series) = ctx
            .quilt_series()
            .into_iter()
            .find(|(path, _)| path == &PathBuf::from("debian/patches/series"))
            .map(|(_, series)| series)
        else {
            return;
        };
        let names: Vec<String> = series
            .patches()
            .iter()
            .map(|patch| patch.name().to_string())
            .collect();
        let Some(files) = ctx.files() else {
            return;
        };
        let templated: Vec<PathBuf> = names
            .iter()
            .filter_map(|name| {
                let path = PathBuf::from("debian/patches").join(name);
                let bytes = readable(files, &path)?;
                has_template_description(&bytes).then_some(path)
            })
            .collect();
        for path in templated {
            ctx.diagnostic(
                Diagnostic::warn("")
                    .with_severity(Severity::Info)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

/// Whether a line before the first `---` line contains the template text.
fn has_template_description(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes);
    text.split('\n')
        .take_while(|line| !line.starts_with("---"))
        .any(|line| line.contains(PATCH_DESC_TEMPLATE))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const SERIES: &str = "01-fix.patch\n";

    fn quilt(patch: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/source/format", TestFile::new("3.0 (quilt)\n")),
            ("debian/patches/series", TestFile::new(SERIES)),
            ("debian/patches/01-fix.patch", TestFile::new(patch)),
        ]
    }

    #[test]
    fn source() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            quilt("Description: fix\n---\n"),
            // The template after the first `---` line is not description.
            quilt(
                "Description: fix\n---\nTODO: Put a short summary on the line above and replace this paragraph\n",
            ),
            // A template that is only a prefix does not match.
            quilt("Description: TODO: Put a short summary\n---\n"),
            vec![
                ("debian/source/format", TestFile::new("1.0\n")),
                ("debian/patches/series", TestFile::new(SERIES)),
                (
                    "debian/patches/01-fix.patch",
                    TestFile::new(
                        "Description: TODO: Put a short summary on the line above and replace this paragraph\n---\n",
                    ),
                ),
            ],
        ];
        let fail = vec![
            quilt(
                "Description: TODO: Put a short summary on the line above and replace this paragraph\n---\n",
            ),
            quilt(
                "TODO: Put a short summary on the line above and replace this paragraph\nDescription: x\n---\n",
            ),
            // The template in a lead-in line still counts.
            quilt(
                "Index: TODO: Put a short summary on the line above and replace this paragraph\n---\n",
            ),
            vec![
                ("debian/source/format", TestFile::new("3.0 (quilt)\n")),
                (
                    "debian/patches/series",
                    TestFile::new("one.patch\ntwo.patch\n"),
                ),
                (
                    "debian/patches/one.patch",
                    TestFile::new(
                        "Subject: TODO: Put a short summary on the line above and replace this paragraph\n---\n",
                    ),
                ),
                (
                    "debian/patches/two.patch",
                    TestFile::new(
                        "Subject: TODO: Put a short summary on the line above and replace this paragraph\n---\n",
                    ),
                ),
            ],
        ];
        SourceTreeTester::new(QuiltPatchUsingTemplateDescription, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(quilt("Description: fix\n---\n"))];
        let fail = vec![SourcePackageCase::from(quilt(
            "Description: TODO: Put a short summary on the line above and replace this paragraph\n---\n",
        ))];
        SourcePackageTester::new(QuiltPatchUsingTemplateDescription, pass, fail)
            .test_and_snapshot();
    }
}
