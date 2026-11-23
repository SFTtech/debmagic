use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};
use crate::lint::spelling::{check_spelling, source_exceptions};

use super::ln0396_format_3_0_but_debian_changes_patch::{is_quilt_format, source_format};
use super::ln0706_patch_file_present_but_not_mentioned_in_series::BUILD_DEPENDS_ALL;
use super::ln1325_quilt_patch_missing_description::readable;

pub struct SpellingErrorInPatchDescription;

declare_rule! {
    /// The description of a patch listed in `debian/patches/series` contains a
    /// misspelling from lintian's spelling lists.
    ///
    /// Runs under the gate of `quilt-patch-with-non-standard-options` (LN1207):
    /// Format is `3.x (quilt)` or `Build-Depends-All` satisfies `quilt`, and
    /// `debian/patches/series` is readable. A patch whose path or description
    /// contains `spelling` or `typo` (case-insensitively) is skipped. The
    /// description is the patch lines before the first `---` line, without
    /// blank lines and lead-in lines (`Index: `, `====`, `diff ...`, `index `,
    /// `From: `). Extra is the misspelling and its correction. The Location is
    /// the patch file with no line. Reported at pedantic severity.
    ///
    /// Exceptions are the source and binary names and the `Maintainer` and
    /// `Uploaders` names of `debian/control` (see `source_exceptions`); lintian
    /// also exempts `Changed-By` and the binary packages' people, which a Source
    /// package does not have. Repeated words are not reported (lintian passes
    /// no duplicate check here). Applies to the Source tree and Source package.
    SpellingErrorInPatchDescription,
    code = "LN1705",
    tag = "spelling-error-in-patch-description",
    default_selected = false,
    experimental = false,
}

static LEAD_IN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:Index: |=+$|diff .+|index |From: )").expect("valid regex"));

impl SourceRule for SpellingErrorInPatchDescription {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let quilt_format = is_quilt_format(&source_format(ctx));
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
        let exceptions = ctx
            .debian_control()
            .map(source_exceptions)
            .unwrap_or_default();
        let Some(files) = ctx.files() else {
            return;
        };
        let mut found = Vec::new();
        for name in names {
            let path = PathBuf::from("debian/patches").join(&name);
            let Some(bytes) = readable(files, &path) else {
                continue;
            };
            let description = description(&bytes);
            let mentions_typo = |text: &str| {
                let lower = text.to_lowercase();
                lower.contains("spelling") || lower.contains("typo")
            };
            if mentions_typo(&path.to_string_lossy()) || mentions_typo(&description) {
                continue;
            }
            for (misspelled, correction) in check_spelling(&description, &exceptions, false) {
                found.push((path.clone(), format!("{misspelled} {correction}")));
            }
        }
        for (path, extra) in found {
            ctx.diagnostic(
                Diagnostic::warn(extra)
                    .with_severity(Severity::Pedantic)
                    .with_location(Location {
                        path,
                        line: None,
                        column: None,
                    }),
            );
        }
    }
}

/// The description lines of a patch: lines before the first `---` line that are
/// not blank and not lead-in, each kept with its newline, as lintian appends them.
fn description(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut description = String::new();
    for line in text.split_inclusive('\n') {
        if line.starts_with("---") {
            break;
        }
        let bare = line.strip_suffix('\n').unwrap_or(line);
        if bare.trim().is_empty() || LEAD_IN.is_match(bare) {
            continue;
        }
        description.push_str(line);
    }
    description
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "\
Source: example
Maintainer: Maintainer <m@example.com>
Build-Depends: debhelper-compat (= 13), quilt

Package: example
Architecture: all
Description: example
 extra
";

    fn tree(series: &str, patches: &[(&'static str, &str)]) -> Vec<(String, TestFile)> {
        let mut files = vec![
            (
                "debian/source/format".to_string(),
                TestFile::new("3.0 (quilt)\n"),
            ),
            ("debian/control".to_string(), TestFile::new(CONTROL)),
            ("debian/patches/series".to_string(), TestFile::new(series)),
        ];
        for (name, body) in patches {
            files.push((format!("debian/patches/{name}"), TestFile::new(*body)));
        }
        files
    }

    #[test]
    fn source() {
        let pass = vec![
            tree(
                "a.patch\n",
                &[("a.patch", "Fix the spelling of the thing\n---\n+x\n")],
            ),
            tree("a.patch\n", &[("a.patch", "Adds the function\n---\n+x\n")]),
            // A patch named with a typo word is skipped.
            tree("typo-fix.patch\n", &[("typo-fix.patch", "speling\n---\n")]),
            // Words after the first `---` line are not description.
            tree("a.patch\n", &[("a.patch", "Good words\n---\nspeling\n")]),
        ];
        let fail = vec![
            tree("a.patch\n", &[("a.patch", "Fix the speling\n---\n+x\n")]),
            tree(
                "a.patch\n",
                &[(
                    "a.patch",
                    "Index: x\n=====\n\nSome speling and a  missing\n---\n",
                )],
            ),
        ];
        SourceTreeTester::new(SpellingErrorInPatchDescription, pass, fail).test_and_snapshot();
    }
}
