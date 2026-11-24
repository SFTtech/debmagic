use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::package_relation::PackageRelation;
use crate::lint::rule::BinaryPackageRule;

use super::shlibs;

pub struct DistantPrerequisiteInShlibs;

declare_rule! {
    /// The shlibs control file lists a dependency the package does not satisfy.
    ///
    /// Only when the package ships a versioned shared library, and its `shlibs`
    /// control file is a regular file. Each dependency of each line that is not
    /// a udeb line is reported, once, when the package's own relation does not
    /// imply it. The relation is the package's `Provides` AND-ed with its name,
    /// plus `(= Version)` when `Version` is declared. Extra is the line's
    /// dependencies joined by single spaces (`libbaz1 (>> 1-1)`), and the
    /// Location is `shlibs` with no line. A line with no dependencies implies
    /// nothing and is quiet. A `shlibs` that is a symlink counts as absent, and
    /// one that is not UTF-8 reads as empty, as lintian's `decoded_utf8` does.
    /// Reads the shlibs text and the package's own fields, not the ELF symbols.
    DistantPrerequisiteInShlibs,
    code = "LN1032",
    tag = "distant-prerequisite-in-shlibs",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DistantPrerequisiteInShlibs {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(text) = shlibs::control_text(ctx) else {
            return;
        };
        if shlibs::versioned_libraries(ctx).is_empty() {
            return;
        }
        let provides = provides_or_self(ctx);
        for prerequisite in shlibs::prerequisites(&text) {
            if !provides.satisfies(&prerequisite) {
                ctx.diagnostic(Diagnostic::warn(prerequisite).with_location(shlibs::at()));
            }
        }
    }
}

/// Lintian's `Provides` AND the package itself, with `(= Version)` when the
/// installation control declares a Version.
fn provides_or_self(ctx: &BinaryPackageContext<'_>) -> PackageRelation {
    let fields = ctx.installation_control().fields();
    let own = if fields.declares("Version") {
        format!(
            "{} (= {})",
            ctx.package_name(),
            fields.unfolded_value("Version").unwrap_or_default()
        )
    } else {
        ctx.package_name().to_string()
    };
    fields
        .relation("Provides")
        .unwrap_or_default()
        .and(&PackageRelation::parse(&own))
}

#[cfg(test)]
mod tests {
    use crate::lint::elf::TestElf;
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn elf(elf: TestElf) -> TestFile {
        TestFile::from_bytes(elf.build())
    }

    fn library() -> (&'static str, TestFile) {
        (
            "usr/lib/libfoo.so.1",
            elf(TestElf::shared_object().soname("libfoo.so.1")),
        )
    }

    fn shlibs(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![library()])
            .package("libfoo1")
            .control_file("shlibs", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            // No versioned shared library: the Check does not run.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .package("libfoo1")
                .control_file("shlibs", TestFile::new("libbar 1 libbaz (>> 1)\n")),
            // The package itself, its Provides, and an empty dependency.
            shlibs("libfoo 1 libfoo1\n"),
            shlibs("libfoo 1 libbar\n").field("Provides", "libbar"),
            shlibs("libfoo 1\n"),
            // A udeb line is not checked; the deb line names its own package.
            shlibs("udeb: libfoo 1 libbar\nlibfoo 1 libfoo1\n"),
            // Comments and blank lines.
            shlibs("# comment\n\nlibfoo 1 libfoo1\n"),
            // A shlibs that is a symlink or not UTF-8 reads as absent or empty.
            BinaryPackageCase::from(vec![library()])
                .package("libfoo1")
                .control_file("shlibs", TestFile::symlink("other")),
            BinaryPackageCase::from(vec![library()])
                .package("libfoo1")
                .control_file(
                    "shlibs",
                    TestFile::from_bytes(b"libfoo 1 libbar\xff\n".to_vec()),
                ),
        ];
        let fail = vec![
            shlibs("libfoo 1 libbar\n"),
            shlibs("libfoo 1 libbaz (>> 1)\n"),
            // The same prerequisite on two lines is reported once.
            shlibs("libfoo 1 libbar\nlibfoo 2 libbar\n"),
            // A udeb line is skipped, a deb line is not.
            shlibs("udeb: libfoo 1 libbar\nlibfoo 1 libbar\n"),
        ];
        BinaryPackageTester::new(DistantPrerequisiteInShlibs, pass, fail).test_and_snapshot();
    }
}
