use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;
use crate::lint::type1::AdobeFragment;

pub struct LicenseProblemFontAdobeCopyrightedFragmentNoCredit;

declare_rule! {
    /// A Type 1 font program contains `UniqueID 6859`, the Adobe Type 1 font
    /// identifier, without an Adobe copyright notice before it (Debian's
    /// type1nondfsg procedure). Reports the font's path with an empty extra.
    ///
    /// Only Type 1 fonts among the regular files and hardlinks are checked; the
    /// scan is the one of `license-problem-font-adobe-copyrighted-fragment`, so
    /// a font whose Adobe fragment completes first is reported there and not
    /// here. The text is lintian's `t1disasm` output (see that Rule for what
    /// differs).
    LicenseProblemFontAdobeCopyrightedFragmentNoCredit,
    code = "LN1394",
    tag = "license-problem-font-adobe-copyrighted-fragment-no-credit",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for LicenseProblemFontAdobeCopyrightedFragmentNoCredit {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(fonts) = ctx.data_type1_fonts() else {
            return;
        };
        let paths: Vec<PathBuf> = fonts
            .iter()
            .filter(|(_, font)| font.adobe_fragment() == Some(AdobeFragment::NoCredit))
            .map(|(path, _)| path.to_path_buf())
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(path: &str, text: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::from_bytes(text.as_bytes()))])
    }

    #[test]
    fn binary() {
        let pass = vec![
            case(
                "usr/share/fonts/type1/good.pfa",
                "%!FontType1-1.0: X\n/FontName /X def\n",
            ),
            // The Adobe fragment completes before the UniqueID line: the
            // fragment Tag reports this font, not the no-credit one.
            case(
                "usr/share/fonts/type1/fragment.pfa",
                "%!FontType1-1.0: X\n% Copyright (c) 1987-1990 Adobe Systems Incorporated.\n% All Rights Reserved.\ntrick { /UniqueID 6859 } def\n",
            ),
            // Not a Type 1 font.
            case("usr/share/doc/notes.txt", "trick { /UniqueID 6859 } def\n"),
        ];
        let fail = vec![
            case(
                "usr/share/fonts/type1/nocredit.pfa",
                "%!FontType1-1.0: X\ntrick { /UniqueID 6859 } def\n",
            ),
            // A copyright line that is not followed by the fragment does not
            // stop the scan.
            case(
                "usr/share/fonts/type1/nocredit2.pfa",
                "%!PS-AdobeFont-1.0: X\n% Copyright (c) 1990-1991 Adobe Systems Incorporated\n% Other.\n/UniqueID 6859 def\n",
            ),
        ];
        BinaryPackageTester::new(
            LicenseProblemFontAdobeCopyrightedFragmentNoCredit,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
