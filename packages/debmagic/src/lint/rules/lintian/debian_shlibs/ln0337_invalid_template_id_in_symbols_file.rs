use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::symbols;

pub struct InvalidTemplateIdInSymbolsFile;

declare_rule! {
    /// A symbol definition names an alternative-dependency template that is
    /// not a number, or a number greater than the templates declared for
    /// that soname.
    ///
    /// Template ids start at 1. `0` is accepted. Extra is the template token,
    /// which may be two words. The Location is `symbols` and the line.
    InvalidTemplateIdInSymbolsFile,
    code = "LN0337",
    tag = "invalid-template-id-in-symbols-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InvalidTemplateIdInSymbolsFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(found) = symbols::inspect(ctx) else {
            return;
        };
        for (line, selector) in found.invalid_templates {
            ctx.diagnostic(Diagnostic::error(selector).with_location(symbols::at(Some(line))));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::BinaryPackageTester;

    use super::super::symbols::{self, LEGACY_DEBUG};
    use super::*;

    #[test]
    fn test() {
        let pass = vec![
            symbols::symbols_case(
                Some("1.0"),
                "\
libfoo.so.1 pkg #MINVER#
| other
 sym@Base 1.0 1
 sym@Base 1.0 0
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
            symbols::symbols_case(Some("1.0"), LEGACY_DEBUG),
            symbols::symbols_case(
                Some("1.0"),
                "\
libfoo.so.1 pkg #MINVER#
 sym@Base 1.0 1
",
            ),
        ];
        BinaryPackageTester::new(InvalidTemplateIdInSymbolsFile, pass, fail).test_and_snapshot();
    }
}
