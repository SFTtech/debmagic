use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::copyright_text;

pub struct CopyrightFileContainsFullApache2License;

const LENGTH: usize = 10_000;

static HEADER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bApache License\s+Version 2\.0,").expect("valid regex"));
static TERMS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION")
        .expect("valid regex")
});

fn contains_full_apache2(contents: &str) -> bool {
    contents.chars().count() > LENGTH && HEADER.is_match(contents) && TERMS.is_match(contents)
}

declare_rule! {
    /// The copyright file is longer than 10000 characters and contains the Apache
    /// 2.0 terms.
    ///
    /// The file must say `Apache License Version 2.0,` and contain `TERMS AND
    /// CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION`. Udebs are not checked.
    /// Extra and Location are empty.
    CopyrightFileContainsFullApache2License,
    code = "LN0605",
    tag = "copyright-file-contains-full-apache-2-license",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CopyrightFileContainsFullApache2License {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(contents) = copyright_text::installed_text(ctx) else {
            return;
        };
        if contains_full_apache2(&contents) {
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

    const APACHE: &str = "Apache License\nVersion 2.0, January 2004\n\nTERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION\n";

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
            BinaryPackageCase::from(installed(&padded(APACHE, 10_000))),
            BinaryPackageCase::from(installed(APACHE)),
            BinaryPackageCase::from(installed(&padded(
                "Apache License\nVersion 2.0\nTERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION\n",
                10_001,
            ))),
            BinaryPackageCase::from(installed(&padded("Apache License Version 2.0,\n", 10_001))),
            BinaryPackageCase::from(installed(&padded(APACHE, 10_001))).udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
        ];
        let fail = vec![
            BinaryPackageCase::from(installed(&padded(APACHE, 10_001))),
            BinaryPackageCase::from(installed(&padded(
                "Apache License   Version 2.0,\nTERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION\n",
                10_001,
            ))),
        ];
        BinaryPackageTester::new(CopyrightFileContainsFullApache2License, pass, fail)
            .test_and_snapshot();
    }
}
