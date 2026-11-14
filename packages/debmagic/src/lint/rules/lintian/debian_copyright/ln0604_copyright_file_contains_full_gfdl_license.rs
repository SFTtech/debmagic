use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::copyright_text;

pub struct CopyrightFileContainsFullGfdlLicense;

const LENGTH: usize = 12_000;

static GFDL_V12: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bGNU Free Documentation License\s*Version 1\.2").expect("valid regex")
});
static APPLICABILITY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b1\. APPLICABILITY AND DEFINITIONS").expect("valid regex"));

fn contains_full_gfdl(contents: &str) -> bool {
    contents.chars().count() > LENGTH
        && GFDL_V12.is_match(contents)
        && APPLICABILITY.is_match(contents)
}

declare_rule! {
    /// The copyright file is longer than 12000 characters and contains the GFDL
    /// 1.2 terms.
    ///
    /// The file must say `GNU Free Documentation License` and `Version 1.2`, and
    /// contain `1. APPLICABILITY AND DEFINITIONS`. Udebs are not checked. Extra and
    /// Location are empty.
    CopyrightFileContainsFullGfdlLicense,
    code = "LN0604",
    tag = "copyright-file-contains-full-gfdl-license",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CopyrightFileContainsFullGfdlLicense {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(contents) = copyright_text::installed_text(ctx) else {
            return;
        };
        if contains_full_gfdl(&contents) {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn installed(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("usr/share/doc/example", TestFile::directory()),
            ("usr/share/doc/example/copyright", TestFile::new(body)),
        ]
    }

    const V12: &str = "GNU Free Documentation License\nVersion 1.2, November 2002\n\n1. APPLICABILITY AND DEFINITIONS\n";

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
            BinaryPackageCase::from(installed(&padded(V12, 12_000))),
            BinaryPackageCase::from(installed(V12)),
            BinaryPackageCase::from(installed(&padded(
                "GNU Free Documentation License\nVersion 1.1\n1. APPLICABILITY AND DEFINITIONS\n",
                12_001,
            ))),
            BinaryPackageCase::from(installed(&padded(
                "GNU Free Documentation License\nVersion 1.2\n",
                12_001,
            ))),
            BinaryPackageCase::from(installed(&padded(V12, 12_001))).udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
        ];
        let fail = vec![
            BinaryPackageCase::from(installed(&padded(V12, 12_001))),
            BinaryPackageCase::from(installed(&padded(
                "GNU Free Documentation License Version 1.2\n1. APPLICABILITY AND DEFINITIONS\n",
                12_001,
            ))),
        ];
        BinaryPackageTester::new(CopyrightFileContainsFullGfdlLicense, pass, fail)
            .test_and_snapshot();
    }
}
