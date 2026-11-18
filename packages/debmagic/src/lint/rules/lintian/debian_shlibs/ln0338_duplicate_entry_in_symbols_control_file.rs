use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::symbols;

pub struct DuplicateEntryInSymbolsControlFile;

declare_rule! {
    /// The same soname header appears more than once in the symbols file.
    ///
    /// Extra is `(lines N N) name version`, with the lines sorted and the
    /// soname split the way `dpkg-shlibdeps` splits it (`libfoo.so.1` is
    /// `libfoo 1`). The Location is `symbols` with no line.
    DuplicateEntryInSymbolsControlFile,
    code = "LN0338",
    tag = "duplicate-entry-in-symbols-control-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DuplicateEntryInSymbolsControlFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(found) = symbols::inspect(ctx) else {
            return;
        };
        for message in found.duplicates {
            ctx.diagnostic(Diagnostic::error(message).with_location(symbols::at(None)));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::BinaryPackageTester;

    use super::super::symbols::{self, LIBFOO};
    use super::*;

    #[test]
    fn test() {
        let pass = vec![symbols::symbols_case(
            Some("1.0-1"),
            "\
libfoo.so.1 pkg #MINVER#
libbar.so.1 pkg #MINVER#
",
        )];
        let fail = vec![
            symbols::symbols_case(Some("1.0-1"), LIBFOO),
            symbols::symbols_case(
                Some("1.0"),
                "\
libfoo-1.2.so pkg #MINVER#
libfoo-1.2.so pkg #MINVER#
",
            ),
        ];
        BinaryPackageTester::new(DuplicateEntryInSymbolsControlFile, pass, fail)
            .test_and_snapshot();
    }
}
