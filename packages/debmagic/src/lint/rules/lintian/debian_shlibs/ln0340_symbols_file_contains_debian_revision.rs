use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::symbols;

pub struct SymbolsFileContainsDebianRevision;

declare_rule! {
    /// A symbol version contains a hyphen, does not end in `~`, and is not
    /// the package version with its Debian revision removed.
    ///
    /// The package's own full version is
    /// `symbols-file-contains-current-version-with-debian-revision` instead.
    /// A missing or empty `Version` is `0-1`. Extra matches that tag. The
    /// Location is `symbols` with no line.
    SymbolsFileContainsDebianRevision,
    code = "LN0340",
    tag = "symbols-file-contains-debian-revision",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SymbolsFileContainsDebianRevision {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(found) = symbols::inspect(ctx) else {
            return;
        };
        for message in found.debian_revisions {
            ctx.diagnostic(Diagnostic::warn(message).with_location(symbols::at(None)));
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
 kept@Base 1.0-1~
 current@Base 1.0-1
",
            ),
            symbols::symbols_case(
                Some("1.0"),
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
                Some("1.0-1"),
                "\
libfoo.so.1 pkg #MINVER#
 z@Base 0.9-1
 a@Base 0.8-2
",
            ),
        ];
        BinaryPackageTester::new(SymbolsFileContainsDebianRevision, pass, fail).test_and_snapshot();
    }
}
