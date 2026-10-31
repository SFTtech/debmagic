use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::copyright_text;

pub struct CopyrightFileContainsFullGplLicense;

declare_rule! {
    /// The copyright file is longer than 12000 characters and contains the
    /// GPL terms.
    ///
    /// GPL 1 and 2 are the `TERMS AND CONDITIONS FOR COPYING, DISTRIBUTION
    /// AND MODIFICATION` block. GPL 3 is `GNU GENERAL PUBLIC LICENSE` plus
    /// `Version 3` and a `TERMS AND CONDITIONS` line. Udebs are not checked.
    /// Extra and Location are empty.
    CopyrightFileContainsFullGplLicense,
    code = "LN0284",
    tag = "copyright-file-contains-full-gpl-license",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CopyrightFileContainsFullGplLicense {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(contents) = copyright_text::installed_text(ctx) else {
            return;
        };
        if copyright_text::contains_full_gpl(&contents) {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    const V2: &str = "GNU GENERAL PUBLIC LICENSE\nTERMS AND CONDITIONS FOR COPYING, DISTRIBUTION AND MODIFICATION\n";
    const V3: &str =
        "GNU GENERAL PUBLIC LICENSE\nVersion 3, 29 June 2007\n\nTERMS AND CONDITIONS\n";

    fn installed(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("usr/share/doc/example", TestFile::directory()),
            ("usr/share/doc/example/copyright", TestFile::new(body)),
        ]
    }

    fn padded(prefix: &str, chars: usize) -> String {
        let mut text = prefix.to_string();
        let have = text.chars().count();
        if have < chars {
            text.extend(std::iter::repeat_n('x', chars - have));
        }
        text
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(installed(&padded(V2, 12_000))),
            BinaryPackageCase::from(installed(V2)),
            BinaryPackageCase::from(installed(V3)),
            BinaryPackageCase::from(installed(&"x".repeat(12_001))),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(installed(&padded(V2, 12_001))).udeb(),
        ];
        let fail = vec![
            BinaryPackageCase::from(installed(&padded(V2, 12_001))),
            BinaryPackageCase::from(installed(&padded(V3, 12_001))),
        ];
        BinaryPackageTester::new(CopyrightFileContainsFullGplLicense, pass, fail)
            .test_and_snapshot();
    }
}
