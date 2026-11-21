use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::shlibs;

pub struct OutdatedRelationInShlibs;

declare_rule! {
    /// The shlibs control file uses an obsolete `<` or `>` in a versioned dependency.
    ///
    /// Only when the package ships a versioned shared library and its `shlibs`
    /// control file is a regular file. Each distinct dependency text of a
    /// non-udeb line (the dependencies joined by single spaces) that has an
    /// opening parenthesis followed by a lone `<` or `>` is reported: `(< 1)` and
    /// `(> 1)`, not `(<< 1)`, `(>> 1)`, `(<= 1)`, or `(>= 1)`. Extra is that text
    /// (`libunshf1 (> 0.1)`), and the Location is `shlibs` with no line. A
    /// `shlibs` that is a symlink counts as absent.
    OutdatedRelationInShlibs,
    code = "LN1035",
    tag = "outdated-relation-in-shlibs",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for OutdatedRelationInShlibs {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(text) = shlibs::control_text(ctx) else {
            return;
        };
        if shlibs::versioned_libraries(ctx).is_empty() {
            return;
        }
        for prerequisite in shlibs::prerequisites(&text) {
            if shlibs::has_obsolete_operator(&prerequisite) {
                ctx.diagnostic(Diagnostic::warn(prerequisite).with_location(shlibs::at()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn elf(elf: TestElf) -> TestFile {
        TestFile::from_bytes(elf.build())
    }

    fn shlibs(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/lib/libfoo.so.1",
            elf(TestElf::shared_object().soname("libfoo.so.1")),
        )])
        .control_file("shlibs", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            // Not versioned: the Check does not run.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("shlibs", TestFile::new("libfoo 1 libbar (> 1)\n")),
            // The strict and inclusive operators are fine.
            shlibs("libfoo 1 libbar (>> 1)\n"),
            shlibs("libfoo 1 libbar (<< 1)\n"),
            shlibs("libfoo 1 libbar (>= 1) libbaz (<= 2)\n"),
            // Only an opening parenthesis followed by a lone operator counts.
            shlibs("libfoo 1 libbar\n"),
            // A udeb line is not checked.
            shlibs("udeb: libfoo 1 libbar (> 1)\n"),
            // A shlibs that is a symlink counts as absent.
            BinaryPackageCase::from(vec![(
                "usr/lib/libfoo.so.1",
                elf(TestElf::shared_object().soname("libfoo.so.1")),
            )])
            .control_file("shlibs", TestFile::symlink("other")),
        ];
        let fail = vec![
            shlibs("libfoo 1 libbar (> 1)\n"),
            shlibs("libfoo 1 libbar (< 1)\n"),
            // Spacing inside the parentheses does not matter.
            shlibs("libfoo 1 libbar (  <  2)\n"),
            // Reported once even on two lines.
            shlibs("libfoo 1 libbar (> 1)\nlibfoo 2 libbar (> 1)\n"),
            // The line's dependencies are one text: an obsolete operator anywhere reports all of it.
            shlibs("libfoo 1 libbar (>= 1) libbaz (> 2)\n"),
        ];
        BinaryPackageTester::new(OutdatedRelationInShlibs, pass, fail).test_and_snapshot();
    }
}
