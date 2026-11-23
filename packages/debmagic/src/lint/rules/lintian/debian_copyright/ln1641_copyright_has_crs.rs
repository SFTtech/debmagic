use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, Severity};

use super::copyright_text;

pub struct CopyrightHasCrs;

declare_rule! {
    /// The copyright file has carriage returns.
    ///
    /// Any `\r` in the installed copyright file (after gunzipping a
    /// `copyright.gz`, and following a symlink). Files that are not UTF-8 are
    /// skipped. Extra and Location are empty. Pedantic, not selected by
    /// default. Udebs are not checked.
    CopyrightHasCrs,
    code = "LN1641",
    tag = "copyright-has-crs",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for CopyrightHasCrs {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(contents) = copyright_text::installed_text(ctx) else {
            return;
        };
        if contents.contains('\r') {
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
            BinaryPackageCase::from(installed("Copyright 2024\r\n")).udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
        ];
        let fail = vec![
            BinaryPackageCase::from(installed("Copyright 2024 Someone\r\n")),
            BinaryPackageCase::from(installed("\r")),
        ];
        BinaryPackageTester::new(CopyrightHasCrs, pass, fail).test_and_snapshot();
    }
}
