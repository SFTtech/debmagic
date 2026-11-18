use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::nmu::nmu_facts;
use super::source::changelog_location;

pub struct SourceNmuHasIncorrectVersionNumber;

declare_rule! {
    /// A source NMU's version is not a single extra Debian revision or
    /// `+nmuN`.
    ///
    /// The same NMU recognition as `no-nmu-in-changelog`. A revision with one
    /// dotted suffix (`-1.1`), or `+nmuN`, is the expected shape. Two dotted
    /// suffixes and `+bN` are not. Backports (`~bpoNuN`) and stable updates
    /// (`~debNuN` or `+debNuN`) are exempt. Extra is the version. The newest
    /// heading is the Location.
    SourceNmuHasIncorrectVersionNumber,
    code = "LN0314",
    tag = "source-nmu-has-incorrect-version-number",
    default_selected = true,
    experimental = false,
}

impl SourceRule for SourceNmuHasIncorrectVersionNumber {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = nmu_facts(ctx) else {
            return;
        };
        if !facts.incorrect_version {
            return;
        }
        ctx.diagnostic(
            Diagnostic::warn(facts.version).with_location(changelog_location(facts.line)),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    const MAINTAINER: &str = "\
Source: example
Maintainer: A Maintainer <a@example.com>

Package: example
Architecture: all
Description: example
 extra
";

    fn entry(version: &str, body: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * {body}\n\n -- B Other <b@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn tree(changelog: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new(MAINTAINER)),
            ("debian/changelog", TestFile::new(changelog)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree(&entry("1.0-1.1", "NMU.")),
            tree(&entry("1.0+nmu1", "NMU.")),
            tree(&entry("1.0-1~deb10u1", "Stable update.")),
            tree(&entry("1.0-1~bpo10+1", "Backport.")),
            tree(&entry("1.0local1", "Local rebuild.")),
            // The maintainer's own upload, even with an NMU-shaped version.
            vec![
                ("debian/control", TestFile::new(MAINTAINER)),
                (
                    "debian/changelog",
                    TestFile::new(
                        "example (1.0-1.1) unstable; urgency=low\n\n  * Mine.\n\n -- A Maintainer <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n",
                    ),
                ),
            ],
        ];
        let fail = vec![
            tree(&entry("1.0", "Native NMU without +nmu.")),
            tree(&entry("1.0-1", "NMU that kept the revision.")),
            tree(&entry("1.5-.3", "Odd revision.")),
            tree(&entry("6ds-1ubuntu0.5.10.1", "Too many revision dots.")),
        ];
        SourceTreeTester::new(SourceNmuHasIncorrectVersionNumber, pass, fail).test_and_snapshot();
    }
}
