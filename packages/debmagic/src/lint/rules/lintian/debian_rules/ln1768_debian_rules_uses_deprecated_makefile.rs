use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::data;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

use super::text::{complete_lines, follow_rules};

pub struct DebianRulesUsesDeprecatedMakefile;

declare_rule! {
    /// `debian/rules` includes a makefile that is known to be deprecated.
    ///
    /// Follows `debian/rules` through symbolic links and, for each logical
    /// line (comments dropped, `\` continuations joined) that starts with
    /// `include`, `-include`, or `sinclude`, reports the first word after it
    /// when the vendored `rules/deprecated-makefiles` table lists that exact
    /// path. Extra is the makefile path. Location is the followed file at the
    /// last physical line of the logical line. Applies to the Source tree and
    /// Source package. The table is empty in lintian 2.144.0, so this Rule
    /// reports nothing with that data; it is ported for when the table gains
    /// entries.
    DebianRulesUsesDeprecatedMakefile,
    code = "LN1768",
    tag = "debian-rules-uses-deprecated-makefile",
    default_selected = true,
    experimental = false,
}

static DEPRECATED: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    data::lines(include_str!("../../../data/rules/deprecated-makefiles")).collect()
});

static INCLUDE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*[s-]?include\s+(\S+)").expect("valid regex"));

impl SourceRule for DebianRulesUsesDeprecatedMakefile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some((path, entry)) = follow_rules(files) else {
            return;
        };
        let Ok(bytes) = files.read(entry) else {
            return;
        };
        let Ok(contents) = String::from_utf8(bytes) else {
            return;
        };
        let hints = deprecated_includes(&contents, |makefile| DEPRECATED.contains(makefile));
        for (line, makefile) in hints {
            ctx.diagnostic(Diagnostic::warn(makefile).with_location(Location {
                path: path.clone(),
                line: Some(line),
                column: None,
            }));
        }
    }
}

/// `(line, makefile)` for each include line whose makefile `is_deprecated`
/// recognizes, in file order.
fn deprecated_includes(contents: &str, is_deprecated: impl Fn(&str) -> bool) -> Vec<(u32, String)> {
    complete_lines(contents)
        .into_iter()
        .filter_map(|(line, text)| {
            let makefile = INCLUDE.captures(&text)?.get(1)?.as_str();
            is_deprecated(makefile).then(|| (line, makefile.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn rules(content: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/rules", TestFile::new(content))]
    }

    fn old(makefile: &str) -> bool {
        makefile == "/usr/share/old/legacy.mk"
    }

    #[test]
    fn includes_are_matched_on_the_logical_line() {
        let contents = "# include /usr/share/old/legacy.mk\n\
                        include /usr/share/old/legacy.mk\n\
                        -include \\\n\
                        \t/usr/share/old/legacy.mk\n\
                        sinclude /usr/share/old/other.mk\n\
                        %:\n\tinclude /usr/share/old/legacy.mk\n";
        assert_eq!(
            deprecated_includes(contents, old),
            vec![
                (2, "/usr/share/old/legacy.mk".to_string()),
                (4, "/usr/share/old/legacy.mk".to_string()),
                // Leading whitespace is allowed before `include`.
                (7, "/usr/share/old/legacy.mk".to_string()),
            ]
        );
    }

    #[test]
    fn includes_need_an_exact_path() {
        assert!(deprecated_includes("include /usr/share/old/legacy.mk.bak\n", old).is_empty());
        assert!(deprecated_includes("includes /usr/share/old/legacy.mk\n", old).is_empty());
    }

    #[test]
    fn test() {
        // The vendored table is empty in lintian 2.144.0, so no include is
        // deprecated and no fail case can be built from the table.
        let pass = vec![
            vec![("README", TestFile::new("no rules"))],
            rules("#!/usr/bin/make -f\ninclude /usr/share/quilt/quilt.make\n"),
            rules("#!/usr/bin/make -f\n-include /usr/share/old/legacy.mk\n"),
            // The link is followed; the target has no deprecated include.
            vec![
                ("debian/rules", TestFile::symlink("real")),
                (
                    "debian/real",
                    TestFile::new("#!/usr/bin/make -f\ninclude /usr/share/old/legacy.mk\n"),
                ),
            ],
            // A dangling link ends the check.
            vec![("debian/rules", TestFile::symlink("missing"))],
        ];
        SourceTreeTester::new(
            DebianRulesUsesDeprecatedMakefile,
            pass,
            Vec::<Vec<(&str, TestFile)>>::new(),
        )
        .test();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(rules(
            "#!/usr/bin/make -f\ninclude /usr/share/quilt/quilt.make\n",
        ))];
        SourcePackageTester::new(
            DebianRulesUsesDeprecatedMakefile,
            pass,
            Vec::<SourcePackageCase>::new(),
        )
        .test();
    }
}
