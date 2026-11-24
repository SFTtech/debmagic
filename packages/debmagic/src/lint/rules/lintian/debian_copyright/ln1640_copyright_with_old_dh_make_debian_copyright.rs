use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::copyright_text;

pub struct CopyrightWithOldDhMakeDebianCopyright;

declare_rule! {
    /// The copyright file has the old dh_make packaging copyright boilerplate.
    ///
    /// `The Debian packaging is (C) <year>` (case-insensitive, whitespace
    /// between the words) in the installed copyright file. `(C)` alone is not a
    /// valid notice in some countries. Extra is empty and the Location is
    /// empty. Pedantic, not selected by default. Udebs are not checked.
    CopyrightWithOldDhMakeDebianCopyright,
    code = "LN1640",
    tag = "copyright-with-old-dh-make-debian-copyright",
    default_selected = false,
    experimental = false,
}

static OLD_DH_MAKE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)The\s+Debian\s+packaging\s+is\s+\(C\)\s+\d+").expect("valid regex")
});

impl BinaryPackageRule for CopyrightWithOldDhMakeDebianCopyright {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(contents) = copyright_text::installed_text(ctx) else {
            return;
        };
        if OLD_DH_MAKE.is_match(&contents) {
            ctx.diagnostic(Diagnostic::warn("").with_severity(Severity::Pedantic));
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

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(installed("Copyright 2024 Someone\n")),
            BinaryPackageCase::from(installed("The Debian packaging is (C) 2024\n")).udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
        ];
        let fail = vec![
            BinaryPackageCase::from(installed("The Debian packaging is (C) 2024 Someone\n")),
            BinaryPackageCase::from(installed("the debian  PACKAGING\tis\n(C) 1999\n")),
        ];
        BinaryPackageTester::new(CopyrightWithOldDhMakeDebianCopyright, pass, fail)
            .test_and_snapshot();
    }
}
