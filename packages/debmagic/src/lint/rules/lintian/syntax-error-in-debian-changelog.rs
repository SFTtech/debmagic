use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceTreeContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceTreeRule;

pub struct SyntaxErrorInDebianChangelog;

declare_rule! {
    /// Debian changelog grammar errors from parsing `debian/changelog`.
    ///
    /// Each parser error is one Diagnostic. Extra is the quoted condition
    /// lintian puts on `syntax-error-in-debian-changelog`. Source-tree
    /// inspects `debian/changelog`.
    SyntaxErrorInDebianChangelog,
    code = "LN0002",
    tag = "syntax-error-in-debian-changelog",
    default_selected = true,
    experimental = false,
}

impl SourceTreeRule for SyntaxErrorInDebianChangelog {
    fn run(&self, ctx: &mut SourceTreeContext<'_>) {
        let Some(changelog) = ctx.debian_changelog() else {
            return;
        };
        let reports: Vec<(u32, String)> = changelog
            .errors()
            .iter()
            .map(|error| (error.line(), error.message().to_string()))
            .collect();

        for (line, message) in reports {
            ctx.diagnostic(
                Diagnostic::warn(format!("\"{message}\"")).with_location(Location {
                    path: PathBuf::from("debian/changelog"),
                    line: Some(line),
                    column: None,
                }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::SourceTreeTester;

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    const VALID_CHANGELOG: &str = concat!(
        "example (1.0) unstable; urgency=low\n",
        "\n",
        "  * test\n",
        "\n",
        " -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 -0800\n",
    );

    #[test]
    fn test() {
        let pass = vec![
            vec![
                ("debian/control", CONTROL),
                ("debian/changelog", VALID_CHANGELOG),
            ],
            vec![("debian/control", CONTROL)],
            vec![
                ("debian/control", CONTROL),
                (
                    "debian/changelog",
                    concat!(
                        "example (1.0) unstable; urgency=low\n",
                        "\n",
                        "  * test\n",
                        "\n",
                        " -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 -0800\n",
                        "\n",
                        "Old Changelog:\n",
                        "\n",
                        "example (0.1) whatever\n",
                    ),
                ),
            ],
        ];
        let fail = vec![
            vec![
                ("debian/control", CONTROL),
                ("debian/changelog", "not a Debian changelog\n"),
            ],
            vec![
                ("debian/control", CONTROL),
                (
                    "debian/changelog",
                    concat!(
                        "example (1.0) unstable; urgency=low\n",
                        "\n",
                        "  * .\n",
                        "  *\n",
                        "\n",
                        " -- A Uthor <a@example.com> Tue, 30 Dec 2008 17:34:02 -0800\n",
                        "\n",
                        "example (0.1) unstable; urgency=low\n",
                        "\n",
                        " -- A Uthor <a@example.com>  Fri, 06 Feb 2009 22:22:37 -0800\n",
                    ),
                ),
            ],
            vec![
                ("debian/control", CONTROL),
                (
                    "debian/changelog",
                    concat!(
                        "example (1.0) unstable; urgency=low\n",
                        "\n",
                        "  * Lintian Test Suite.\n",
                        "  * Test: changelog-file-syntax\n",
                        "\n",
                        " -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 -0800\n",
                        "\n",
                        "example (0.9) unstable; urgency=low\n",
                        "\n",
                        "  * Lintian Test Suite.\n",
                        "  * Test: changelog-file-syntax\n",
                        "\n",
                        " -- A Uthor <a@example.com>  Sat, 09 Apr 2016 10:56:49 +0000\n",
                        "\n",
                        "example () unstable; urgency=low\n",
                        "\n",
                        "  * Lintian Test Suite.\n",
                        "  * Test: changelog-file-syntax\n",
                        "\n",
                        "  * Suppress \"should close ITP bug\" messages.  (Closes: #123456)\n",
                        "\n",
                        " -- A Uthor <a@example.com>  Sat, 02 Apr 2016 10:56:49 +0000\n",
                    ),
                ),
            ],
            vec![
                ("debian/control", CONTROL),
                (
                    "debian/changelog",
                    concat!(
                        "example (1.0) unstable; urgency=low\n",
                        "\n",
                        "  * Lintian Test Suite.\n",
                        "  * Test: changelog-file-strange-date\n",
                        "\n",
                        " -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 -0800\n",
                        "\n",
                        "example (1.0) unstable; urgency=low\n",
                        "\n",
                        "  * The date will fail with some dpkg version see #794674\n",
                        "\n",
                        " -- A Uthor <a@example.com>  The, 15 Apr 2004 23:33:51 +0200\n",
                    ),
                ),
            ],
        ];
        SourceTreeTester::new(SyntaxErrorInDebianChangelog, pass, fail).test_and_snapshot();
    }
}
