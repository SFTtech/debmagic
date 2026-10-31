use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::shlibs;

pub struct DuplicateInShlibs;

declare_rule! {
    /// The shlibs control file repeats a soname line.
    ///
    /// The key is `udeb:` (when present), the library name, and the version.
    /// Dependencies after that are ignored. Extra is `(lines N N)` and that
    /// key. The Location is `shlibs` with no line. An ELF SONAME is not
    /// required.
    DuplicateInShlibs,
    code = "LN0341",
    tag = "duplicate-in-shlibs",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DuplicateInShlibs {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(messages) = shlibs::inspect(ctx) else {
            return;
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message).with_location(shlibs::at()));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::super::shlibs::LIBBAZ_SHLIBS;
    use super::*;

    fn shlibs_file(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("shlibs", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            shlibs_file(
                "\
# comment

libfoo 1 pkg (>= 1)
udeb: libfoo 1 other
",
            ),
            shlibs_file(""),
            shlibs_file("libfoo 1 pkg\n").control_file("shlibs", TestFile::symlink("other")),
        ];
        let fail = vec![
            shlibs_file(LIBBAZ_SHLIBS),
            shlibs_file(
                "\
libfoo 1 first
libbar 2 only
libfoo 1 second
",
            )
            .udeb(),
        ];
        BinaryPackageTester::new(DuplicateInShlibs, pass, fail).test_and_snapshot();
    }
}
