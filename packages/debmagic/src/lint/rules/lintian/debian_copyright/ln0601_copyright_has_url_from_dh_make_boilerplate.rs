use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::copyright_text;

pub struct CopyrightHasUrlFromDhMakeBoilerplate;

declare_rule! {
    /// The copyright file still has the `url://` placeholder from dh_make.
    ///
    /// Any `url://` in the installed copyright file. Udebs are not checked. Extra and
    /// Location are empty.
    CopyrightHasUrlFromDhMakeBoilerplate,
    code = "LN0601",
    tag = "copyright-has-url-from-dh_make-boilerplate",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CopyrightHasUrlFromDhMakeBoilerplate {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(contents) = copyright_text::installed_text(ctx) else {
            return;
        };
        if contents.contains("url://") {
            ctx.diagnostic(Diagnostic::warn(""));
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
            BinaryPackageCase::from(installed("Source: https://example.org/\n")),
            BinaryPackageCase::from(installed("It was downloaded from url://example.org\n")).udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
        ];
        let fail = vec![
            BinaryPackageCase::from(installed("It was downloaded from url://example.org\n")),
            BinaryPackageCase::from(installed("Copyright 2024\nurl://\n")),
        ];
        BinaryPackageTester::new(CopyrightHasUrlFromDhMakeBoilerplate, pass, fail)
            .test_and_snapshot();
    }
}
