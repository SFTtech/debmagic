use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::copyright_text;

pub struct CopyrightContainsAutomaticallyExtractedBoilerplate;

declare_rule! {
    /// The copyright file says it was automatically extracted.
    ///
    /// The literal text `This copyright info was automatically extracted` anywhere in the
    /// installed copyright file. Udebs are not checked. Extra and Location are empty.
    CopyrightContainsAutomaticallyExtractedBoilerplate,
    code = "LN0600",
    tag = "copyright-contains-automatically-extracted-boilerplate",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CopyrightContainsAutomaticallyExtractedBoilerplate {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(contents) = copyright_text::installed_text(ctx) else {
            return;
        };
        if contents.contains("This copyright info was automatically extracted") {
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
            BinaryPackageCase::from(installed("Copyright 2024 Someone\n")),
            BinaryPackageCase::from(installed(
                "This copyright info was automatically extracted\n",
            ))
            .udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
        ];
        let fail = vec![
            BinaryPackageCase::from(installed(
                "This copyright info was automatically extracted\nfrom the upstream sources.\n",
            )),
            BinaryPackageCase::from(installed(
                "Copyright 2024\n\nThis copyright info was automatically extracted",
            )),
        ];
        BinaryPackageTester::new(
            CopyrightContainsAutomaticallyExtractedBoilerplate,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
