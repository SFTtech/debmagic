use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;
use crate::lint::source_format;

pub struct UnsupportedSourceFormat;

declare_rule! {
    /// The source package's format is not `1.0`, `3.0 (quilt)`, or
    /// `3.0 (native)`. Extra is the unfolded format value.
    ///
    /// Source package: the Dsc `Format` field. Source tree: the first line of
    /// `debian/source/format`, as the other Format rules read it (a tree has no
    /// Dsc). A Format that is not declared, or a
    /// tree with no readable `debian/source/format`, is not this Tag. An
    /// empty declared value is reported with an empty extra. Surrounding
    /// spaces are allowed, and so is any run of spaces before the parenthesis,
    /// as lintian allows. `2.0`, `3.0 (git)`, and `1.0 (native)` are not
    /// supported. The `.changes` half of lintian's Check is not ported.
    UnsupportedSourceFormat,
    code = "LN1052",
    tag = "unsupported-source-format",
    default_selected = true,
    experimental = false,
}

static SUPPORTED_FORMAT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?:1\.0|3\.0\s*\((?:quilt|native)\))\s*$").expect("valid regex")
});

impl SourceRule for UnsupportedSourceFormat {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(format) = source_format::field(ctx) else {
            return;
        };
        if !SUPPORTED_FORMAT.is_match(&format) {
            ctx.diagnostic(Diagnostic::error(format));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn tree(format: Option<&str>) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![(
            "debian/control",
            TestFile::new(
                "Source: example\nMaintainer: Example <ex@example.com>\n\
                 Standards-Version: 4.7.2\nSection: devel\n\
                 \nPackage: example\nArchitecture: all\nDescription: example\n extra\n",
            ),
        )];
        if let Some(format) = format {
            files.push(("debian/source/format", TestFile::new(format!("{format}\n"))));
        }
        files
    }

    fn dsc(format: &str) -> Dsc {
        Dsc::parse(&format!("Source: example\n{format}")).expect("Dsc fixture")
    }

    #[test]
    fn source_tree() {
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            tree(None),
            tree(Some("3.0 (quilt)")),
            tree(Some("1.0")),
            tree(Some("3.0 (native)")),
        ];
        let fail = vec![
            tree(Some("2.0")),
            tree(Some("3.0 (git)")),
            tree(Some("1.0 (native)")),
        ];
        SourceTreeTester::new(UnsupportedSourceFormat, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let bare = Vec::<(&str, TestFile)>::new();
        let pass = vec![
            SourcePackageCase::from(bare.clone()).dsc(dsc("")),
            SourcePackageCase::from(bare.clone()).dsc(dsc("Format: 1.0\n")),
            SourcePackageCase::from(bare.clone()).dsc(dsc("Format: 3.0 (native)\n")),
            SourcePackageCase::from(bare.clone()).dsc(dsc("Format:  3.0   (quilt)  \n")),
        ];
        let fail = vec![
            SourcePackageCase::from(bare.clone()).dsc(dsc("Format: 2.0\n")),
            SourcePackageCase::from(bare.clone()).dsc(dsc("Format: 3.0 (git)\n")),
            SourcePackageCase::from(bare.clone()).dsc(dsc("Format: 1.0 (native)\n")),
            SourcePackageCase::from(bare.clone()).dsc(dsc("Format: 1.0\n 1.0\n")),
            SourcePackageCase::from(bare).dsc(dsc("Format:\n")),
        ];
        SourcePackageTester::new(UnsupportedSourceFormat, pass, fail).test_and_snapshot();
    }
}
