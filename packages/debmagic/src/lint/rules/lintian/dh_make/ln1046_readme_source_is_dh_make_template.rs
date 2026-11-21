use std::sync::LazyLock;

use regex::bytes::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct ReadmeSourceIsDhMakeTemplate;

declare_rule! {
    /// The `debian/README.source` file is the unmodified `dh_make` template.
    ///
    /// Reads `debian/README.source` of the packaging File tree (Source tree,
    /// or Source package Patched files) when it is a regular file or a
    /// hardlink. Fails when its bytes contain `You WILL either need to modify
    /// or delete this file`, matched case-insensitively. Extra is empty and
    /// the Location is the path, with no line. A symlink is skipped, as
    /// lintian's `is_file` is false for it. The `file-contains-fixme-placeholder`
    /// half of lintian's `dh-make` Check is a separate Tag and is not done here.
    ReadmeSourceIsDhMakeTemplate,
    code = "LN1046",
    tag = "readme-source-is-dh_make-template",
    default_selected = true,
    experimental = false,
}

const README: &str = "debian/README.source";

static TEMPLATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i-u)You WILL either need to modify or delete this file").expect("valid regex")
});

impl SourceRule for ReadmeSourceIsDhMakeTemplate {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.files() else {
            return;
        };
        let Some(entry) = files.get(README) else {
            return;
        };
        if !entry.is_file() {
            return;
        }
        let Ok(bytes) = files.read(entry) else {
            return;
        };
        if TEMPLATE.is_match(&bytes) {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path: README.into(),
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const TEMPLATE_TEXT: &str = "\
cruft-readme-source for Debian
------------------------------

<this file describes information about the source package, see Debian policy
manual section 4.14. You WILL either need to modify or delete this file>
";

    /// A packaging tree with `debian/control` and the given extra files.
    fn tree(extra: Vec<(&str, TestFile)>) -> Vec<(String, TestFile)> {
        let mut out = vec![(
            "debian/control".to_string(),
            TestFile::new("Source: example\n"),
        )];
        out.extend(
            extra
                .into_iter()
                .map(|(path, file)| (path.to_string(), file)),
        );
        out
    }

    #[test]
    fn source_tree() {
        let pass = vec![
            tree(vec![]),
            tree(vec![(
                "debian/README.source",
                TestFile::new("Our own notes.\n"),
            )]),
            // Another file with the template text is not this check's file.
            tree(vec![("debian/README", TestFile::new(TEMPLATE_TEXT))]),
            // A symlink is not a file.
            tree(vec![(
                "debian/README.source",
                TestFile::symlink("elsewhere"),
            )]),
        ];
        let fail = vec![
            tree(vec![("debian/README.source", TestFile::new(TEMPLATE_TEXT))]),
            // Case-insensitive.
            tree(vec![(
                "debian/README.source",
                TestFile::new("you will EITHER need to modify or delete this FILE\n"),
            )]),
            // A hardlink is a file.
            tree(vec![(
                "debian/README.source",
                TestFile::new(TEMPLATE_TEXT).hardlink(),
            )]),
        ];
        SourceTreeTester::new(ReadmeSourceIsDhMakeTemplate, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(vec![(
            "debian/control",
            TestFile::new("Source: example\n"),
        )])];
        let fail = vec![SourcePackageCase::from(vec![(
            "debian/README.source",
            TestFile::new(TEMPLATE_TEXT),
        )])];
        SourcePackageTester::new(ReadmeSourceIsDhMakeTemplate, pass, fail).test_and_snapshot();
    }
}
