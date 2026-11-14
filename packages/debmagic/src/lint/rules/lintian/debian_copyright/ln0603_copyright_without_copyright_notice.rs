use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{copyright_text, installed_copyright};

pub struct CopyrightWithoutCopyrightNotice;

static COPYRIGHT_NOTICE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:Copyright|Copr\.|\x{A9})(?:.*|[(C):\s]+)\b\d{4}\b|\bpublic(?:\s+|-)domain\b",
    )
    .expect("valid regex")
});

declare_rule! {
    /// The copyright file has no copyright notice.
    ///
    /// A notice is `Copyright`, `Copr.` or the copyright sign followed by a
    /// four-digit year on the same line (case-insensitive; `(C)` and colons
    /// may sit between), or the words public domain. A copyright file that is
    /// a symlink is not judged. Udebs are not checked. Extra and Location are
    /// empty.
    CopyrightWithoutCopyrightNotice,
    code = "LN0603",
    tag = "copyright-without-copyright-notice",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CopyrightWithoutCopyrightNotice {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(contents) = copyright_text::installed_text(ctx) else {
            return;
        };
        let package = ctx.package_name().to_string();
        let Some(files) = ctx.data_files() else {
            return;
        };
        if !installed_copyright::inspect(files, &package)
            .symlink
            .is_empty()
        {
            return;
        }
        if !COPYRIGHT_NOTICE.is_match(&contents) {
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
            BinaryPackageCase::from(installed("COPYRIGHT (C) 1999-2001 Someone\n")),
            BinaryPackageCase::from(installed("Copyright: (C) 2020\n")),
            // Whitespace between the word and the year may include a newline.
            BinaryPackageCase::from(installed("Copyright\n2024\n")),
            BinaryPackageCase::from(installed("Copr. 2001 Someone\n")),
            BinaryPackageCase::from(installed("\u{a9} 2001 Someone\n")),
            BinaryPackageCase::from(installed("Released into the public domain.\n")),
            BinaryPackageCase::from(installed("Released into the Public-Domain.\n")),
            BinaryPackageCase::from(installed("No notice here\n")).udeb(),
            // A symlinked copyright file is not judged.
            BinaryPackageCase::from(vec![
                ("usr/share/doc/example", TestFile::directory()),
                (
                    "usr/share/doc/example/copyright",
                    TestFile::symlink("../other/copyright"),
                ),
            ]),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
        ];
        let fail = vec![
            BinaryPackageCase::from(installed("Some license text\n")),
            BinaryPackageCase::from(installed("Copyright Someone\n")),
            // The year must have four digits.
            BinaryPackageCase::from(installed("Copyright 24\n")),
            BinaryPackageCase::from(installed("")),
        ];
        BinaryPackageTester::new(CopyrightWithoutCopyrightNotice, pass, fail).test_and_snapshot();
    }
}
