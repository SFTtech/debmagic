use std::collections::BTreeMap;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::lintian_tags::current_tag_name;
use crate::lint::rule::SourceRule;

pub struct DuplicateOverrideContext;

declare_rule! {
    /// Two or more overrides in the Source override file name the same Tag
    /// with the same context.
    ///
    /// Grouped by the current Tag name (an old name counts as its renamed
    /// Tag) and the context after the Tag, whitespace collapsed. Extra is the
    /// Tag and `(lines …)` with the 1-based line numbers sorted as text, as
    /// lintian sorts them. The override file is the Location; it has no line.
    DuplicateOverrideContext,
    code = "LN0840",
    tag = "duplicate-override-context",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DuplicateOverrideContext {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(overrides) = ctx.lintian_overrides() else {
            return;
        };
        let mut groups: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
        for entry in overrides.overrides() {
            let tag = current_tag_name(entry.tag()).unwrap_or(entry.tag());
            groups
                .entry((tag.to_string(), entry.context().to_string()))
                .or_default()
                .push(entry.line().to_string());
        }
        for ((tag, _), mut lines) in groups {
            if lines.len() < 2 {
                continue;
            }
            lines.sort();
            ctx.diagnostic(
                Diagnostic::warn(format!("{tag} (lines {})", lines.join(" "))).with_location(
                    Location {
                        path: overrides.path().to_path_buf(),
                        line: None,
                        column: None,
                    },
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    fn tree(overrides: &str) -> Vec<(String, TestFile)> {
        vec![
            ("debian/control".to_string(), TestFile::from(CONTROL)),
            (
                "debian/source/lintian-overrides".to_string(),
                TestFile::from(overrides),
            ),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            tree("example: foo\n"),
            tree("example: foo\nexample: foo bar\n"),
            tree("foo a\nbar a\n"),
            tree("foo a\nfoo b\n"),
            // Same context, different tags.
            tree("foo x\nfoo[amd64]: x\n"),
        ];
        let fail = vec![
            tree("cute-field a b\ncute-field a b\n"),
            tree("cute-field a\ncute-field   a\n"),
            tree("cute-field\ncute-field\ncute-field\n"),
            // Renamed Tag and its current name are one Tag.
            tree("package-install-apt-sources x\npackage-installs-apt-sources x\n"),
            // Lines are sorted as text, as lintian does.
            tree(&format!("\na x\n{}a x\n", "\n".repeat(7))),
        ];
        SourceTreeTester::new(DuplicateOverrideContext, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(tree("foo a\nfoo b\n"))];
        let fail = vec![SourcePackageCase::from(tree("foo a\nfoo a\n"))];
        SourcePackageTester::new(DuplicateOverrideContext, pass, fail).test_and_snapshot();
    }
}
