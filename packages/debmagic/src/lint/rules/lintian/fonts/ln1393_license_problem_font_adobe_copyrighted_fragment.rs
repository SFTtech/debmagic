use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;
use crate::lint::type1::AdobeFragment;

pub struct LicenseProblemFontAdobeCopyrightedFragment;

declare_rule! {
    /// A Type 1 font program contains a fragment of a non-free Adobe license:
    /// a `Copyright (c) 19xx-19xx Adobe Systems Incorporated.` line followed,
    /// in a later line, by `All Rights Reserved.` (Debian's type1nondfsg
    /// procedure). Reports the font's path with an empty extra.
    ///
    /// Only Type 1 fonts (`%!PS-AdobeFont`, `%!FontType1`, or a PFB) among the
    /// regular files and hardlinks are checked. The text is lintian's
    /// `t1disasm` output: the cleartext, then the `eexec` section decrypted.
    /// Fragments that appear only in charstrings are not seen.
    ///
    /// Differs from lintian: the decryption is in debmagic (no `t1disasm`), the
    /// text is decoded as UTF-8 with replacement characters where lintian tries
    /// cp1252 and then UTF-8, and a `UniqueID 6859` line ends the scan for
    /// `license-problem-font-adobe-copyrighted-fragment-no-credit` instead.
    LicenseProblemFontAdobeCopyrightedFragment,
    code = "LN1393",
    tag = "license-problem-font-adobe-copyrighted-fragment",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for LicenseProblemFontAdobeCopyrightedFragment {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(fonts) = ctx.data_type1_fonts() else {
            return;
        };
        let paths: Vec<PathBuf> = fonts
            .iter()
            .filter(|(_, font)| font.adobe_fragment() == Some(AdobeFragment::CopyrightedFragment))
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
        let copyright = "%!FontType1-1.0: X\n% Copyright (c) 1987-1990 Adobe Systems Incorporated.\n% All Rights Reserved.\n";
        let pass = vec![
            // Copyright without the fragment's second line.
            case(
                "usr/share/fonts/type1/a.pfa",
                "%!FontType1-1.0: X\n% Copyright (c) 1987-1990 Adobe Systems Incorporated.\n",
            ),
            // Not a Type 1 font: the text has no font header.
            case(
                "usr/share/doc/notes.txt",
                "% Copyright (c) 1987-1990 Adobe Systems Incorporated.\n% All Rights Reserved.\n",
            ),
            // The no-credit fragment is another Tag.
            case(
                "usr/share/fonts/type1/b.pfa",
                "%!FontType1-1.0: X\ntrick { /UniqueID 6859 } def\n",
            ),
        ];
        let fail = vec![
            case("usr/share/fonts/type1/bad.pfa", copyright),
            case(
                "usr/share/fonts/type1/bad2.pfa",
                "%!PS-AdobeFont-1.0: X\n% Copyright (c) 1990-1991 Adobe Systems Incorporated\n\n% All Rights Reserved\n",
            ),
        ];
        BinaryPackageTester::new(LicenseProblemFontAdobeCopyrightedFragment, pass, fail)
            .test_and_snapshot();
    }
}
