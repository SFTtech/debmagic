use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::symbols;

pub struct SymbolsFileContainsCurrentVersionWithDebianRevision;

declare_rule! {
    /// A symbol is versioned with the package's exact `Version`, and that
    /// version contains a Debian revision.
    ///
    /// A missing or empty `Version` is `0-1`. Extra is `on symbol NAME` plus
    /// ` and N others` when more than one distinct symbol is affected, then
    /// the raw soname in parentheses. The Location is `symbols` with no line.
    SymbolsFileContainsCurrentVersionWithDebianRevision,
    code = "LN0339",
    tag = "symbols-file-contains-current-version-with-debian-revision",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SymbolsFileContainsCurrentVersionWithDebianRevision {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(found) = symbols::inspect(ctx) else {
            return;
        };
        for message in found.current_versions {
            ctx.diagnostic(Diagnostic::error(message).with_location(symbols::at(None)));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::BinaryPackageTester;

    use super::super::symbols::{self, LIBBAZ, LIBFOO};
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            symbols::symbols_case(
                Some("1.0-1"),
                "\
libfoo.so.1 pkg #MINVER#
 sym@Base 1.0
 kept@Base 1.0-3~
",
            ),
            symbols::symbols_case(
                Some("1.0"),
                "\
libfoo.so.1 pkg #MINVER#
 sym@Base 1.0
",
            ),
            symbols::symbols_case(
                None,
                "\
libfoo.so.1 pkg #MINVER#
 sym@Base 1.0
",
            ),
        ];
        let fail = vec![
            symbols::symbols_case(Some("1.0-1"), LIBFOO),
            symbols::symbols_case(Some("1-1"), LIBBAZ),
            symbols::symbols_case(
                Some("2.0-1"),
                "\
libfoo.so.1 pkg #MINVER#
 b@Base 2.0-1
 a@Base 2.0-1
",
            ),
            symbols::symbols_case(
                None,
                "\
libfoo.so.1 pkg #MINVER#
 sym@Base 0-1
",
            ),
        ];
        BinaryPackageTester::new(
            SymbolsFileContainsCurrentVersionWithDebianRevision,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
