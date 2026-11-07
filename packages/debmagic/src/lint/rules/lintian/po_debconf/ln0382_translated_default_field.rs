use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::files;

pub struct TranslatedDefaultField;

declare_rule! {
    /// A debconf template sets `_Default` or `_DefaultChoice` without telling
    /// translators it is translatable.
    ///
    /// Direct children of `debian/` named `templates` (a leading `name.` and a
    /// trailing suffix are allowed). `templates.xx` and `templates.xx_YY` are
    /// language copies and are not checked. A `#` comment mentioning
    /// `translators` immediately above the field suppresses the tag. A value
    /// containing `[` is not this tag. Extra is empty. The Location is the
    /// field line.
    TranslatedDefaultField,
    code = "LN0382",
    tag = "translated-default-field",
    default_selected = true,
    experimental = false,
}

static DEFAULT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^_Default(?:Choice)?: [^\[]*$").expect("valid regex"));
static TRANSLATORS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)translators").expect("valid regex"));

impl SourceRule for TranslatedDefaultField {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(tree) = ctx.files() else {
            return;
        };
        let templates = files::template_files(tree);
        let mut hits = Vec::new();
        for entry in templates {
            let path = files::path_buf(entry);
            let Some(bytes) = files::read(tree, &path) else {
                continue;
            };
            let text = String::from_utf8_lossy(&bytes);
            let mut saw_note = false;
            for (index, line) in text.lines().enumerate() {
                let line_number = u32::try_from(index + 1).unwrap_or(u32::MAX);
                if DEFAULT.is_match(line) && !saw_note {
                    hits.push((path.clone(), line_number));
                }
                if line.starts_with('#') {
                    if TRANSLATORS.is_match(line) {
                        saw_note = true;
                    }
                    continue;
                }
                saw_note = false;
            }
        }
        for (path, line) in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: Some(line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn tree(name: &str, body: &str) -> Vec<(String, TestFile)> {
        vec![
            (
                "debian/control".to_string(),
                TestFile::from(
                    "Source: example\n\nPackage: example\nArchitecture: all\nDescription: example\n extra\n",
                ),
            ),
            (format!("debian/{name}"), TestFile::new(body)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            tree("templates", "Template: example/foo\n"),
            tree(
                "templates",
                "# translators: this is translated\n_Default: yes\n",
            ),
            tree("templates", "_Default: [yes, no]\n"),
            tree("templates.en", "_Default: yes\n"),
            tree("templates.en_US", "_Default: yes\n"),
        ];
        let fail = vec![
            tree("templates", "Template: example/foo\n_Default: yes\n"),
            tree("pkg.templates", "_DefaultChoice: yes\n"),
            tree("templates", "# note\n\n_Default: yes\n"),
        ];
        SourceTreeTester::new(TranslatedDefaultField, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree("templates", "Template: a\n"))];
        let fail = vec![SourcePackageCase::from(tree(
            "templates",
            "_Default: yes\n",
        ))];
        SourcePackageTester::new(TranslatedDefaultField, pass, fail).test_and_snapshot();
    }
}
