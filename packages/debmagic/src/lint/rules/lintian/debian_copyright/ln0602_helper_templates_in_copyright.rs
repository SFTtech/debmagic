use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::copyright_text;

pub struct HelperTemplatesInCopyright;

static HELPER_TEMPLATES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"<fill in (?:http/)?ftp site>",
        r"|<Must follow here>",
        r"|<Put the license of the package here",
        r"|<put author['(]s\)? name and email here>",
        r"|<Copyright \(C\) YYYY Name OfAuthor>",
        r"|Upstream Author\(s\)",
        r"|<years>",
        r"|<special license>",
        r"|<This follows the format of Description: lines\s*in control file>",
        r"|<Including paragraphs>",
        r"|<likewise for another author>",
    ))
    .expect("valid regex")
});

declare_rule! {
    /// The copyright file still has a placeholder from a packaging helper.
    ///
    /// One of the dh_make and dh-make-perl template phrases (`<years>`,
    /// `<special license>`, `Upstream Author(s)` and similar), or
    /// `<INSERT COPYRIGHT YEAR(S) HERE>`. A file with both kinds is reported twice,
    /// once for each check. Udebs are not checked. Extra and Location are empty.
    HelperTemplatesInCopyright,
    code = "LN0602",
    tag = "helper-templates-in-copyright",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for HelperTemplatesInCopyright {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(contents) = copyright_text::installed_text(ctx) else {
            return;
        };
        if HELPER_TEMPLATES.is_match(&contents) {
            ctx.diagnostic(Diagnostic::error(""));
        }
        if contents.contains("<INSERT COPYRIGHT YEAR(S) HERE>") {
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

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(installed("Copyright 2024 Someone\n")),
            BinaryPackageCase::from(installed("<years>\n")).udeb(),
            BinaryPackageCase::from(installed("<Years>\n")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
        ];
        let fail = vec![
            BinaryPackageCase::from(installed("Copyright <years> Someone\n")),
            BinaryPackageCase::from(installed(
                "<Put the license of the package here indented by 1 space>\n",
            )),
            BinaryPackageCase::from(installed("<put author's name and email here>\n")),
            BinaryPackageCase::from(installed("Upstream Author(s)\n")),
            BinaryPackageCase::from(installed("<INSERT COPYRIGHT YEAR(S) HERE>\n")),
            BinaryPackageCase::from(installed("<years> <INSERT COPYRIGHT YEAR(S) HERE>\n")),
        ];
        BinaryPackageTester::new(HelperTemplatesInCopyright, pass, fail).test_and_snapshot();
    }
}
