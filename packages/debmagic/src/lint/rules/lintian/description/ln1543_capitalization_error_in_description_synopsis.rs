use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};
use crate::lint::spelling::check_spelling_picky;

use super::description_field::description;

pub struct CapitalizationErrorInDescriptionSynopsis;

declare_rule! {
    /// The synopsis (first line of `Description`) has a word that lintian's
    /// picky list of capitalization errors corrects, such as `debian` for
    /// `Debian`.
    ///
    /// Lintian's `fields/description` Check, on Installation control, with
    /// `check_spelling_picky` on the synopsis as written. Not checked for an
    /// auto-generated package (one that declares `Auto-Built-Package`, such as
    /// `-dbgsym`), as lintian does. The text `meta package` is reported as
    /// `metapackage`. Each misspelled word is reported with extra
    /// `<word> <correction>`. Location is empty. Udebs are checked too.
    CapitalizationErrorInDescriptionSynopsis,
    code = "LN1543",
    tag = "capitalization-error-in-description-synopsis",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for CapitalizationErrorInDescriptionSynopsis {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx
            .installation_control()
            .fields()
            .declares("Auto-Built-Package")
        {
            return;
        }
        let Some(description) = description(ctx) else {
            return;
        };
        if description.synopsis.is_empty() {
            return;
        }
        for (word, correction) in check_spelling_picky(&description.synopsis) {
            ctx.diagnostic(
                Diagnostic::warn(format!("{word} {correction}")).with_severity(Severity::Info),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn synopsis(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Description", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            synopsis("Debian tools\n A set of tools."),
            // Auto-generated packages are exempt.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Description", "debian debug symbols\n x")
                .field("Auto-Built-Package", "debug-symbols"),
            // The extended description is not part of this Tag.
            synopsis("Tool\n Written for debian systems."),
            // Bracketed text in the synopsis is not checked.
            synopsis("[debian] tools\n A set of tools."),
        ];
        let fail = vec![
            synopsis("debian tools\n A set of tools."),
            synopsis("A meta package for debian\n A tool."),
            synopsis("debian tools\n A set of tools.").udeb(),
            synopsis("dbus helper\n A tool."),
        ];
        BinaryPackageTester::new(CapitalizationErrorInDescriptionSynopsis, pass, fail)
            .test_and_snapshot();
    }
}
