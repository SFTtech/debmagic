use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::symbols;

pub struct SyntaxErrorInSymbolsFile;

declare_rule! {
    /// A symbols control file line is not a soname header, alternative
    /// template, meta field, or symbol definition, or it is one of those out
    /// of order.
    ///
    /// Blank lines and `#` comments are skipped. Extra is empty. The Location
    /// is `symbols` and the 1-based line. Udebs are checked.
    SyntaxErrorInSymbolsFile,
    code = "LN0336",
    tag = "syntax-error-in-symbols-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for SyntaxErrorInSymbolsFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(found) = symbols::inspect(ctx) else {
            return;
        };
        for line in found.syntax_errors {
            ctx.diagnostic(Diagnostic::error("").with_location(symbols::at(Some(line))));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageTester, TestFile};

    use super::super::symbols::{self, LEGACY_DEBUG};
    use super::*;

    const VALID: &str = "\
libfoo.so.1 libfoo1 #MINVER#
| libfoo1c2 (>= 1.2)
* Build-Depends-Package: libfoo1
 foo@Base 1.0
";

    #[test]
    fn test() {
        let pass = vec![
            symbols::symbols_case(Some("1.0-1"), VALID),
            symbols::symbols_case(Some("1.0-1"), ""),
            symbols::symbols_case(Some("1.0-1"), "# only a comment\n\n"),
            symbols::symbols_case(Some("1.0-1"), VALID)
                .control_file("symbols", TestFile::symlink("other")),
            symbols::symbols_case(Some("1.0-1"), VALID)
                .control_file("symbols", TestFile::directory()),
        ];
        let fail = vec![
            symbols::symbols_case(Some("1.0"), LEGACY_DEBUG),
            symbols::symbols_case(Some("1.0-1"), "not a symbols line\n").udeb(),
        ];
        BinaryPackageTester::new(SyntaxErrorInSymbolsFile, pass, fail).test_and_snapshot();
    }
}
