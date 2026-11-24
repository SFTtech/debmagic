use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::super::mailbox::parse_mailbox;

pub struct WrongTeam;

declare_rule! {
    /// The Maintainer is a known team's address, but the source package name
    /// contains another team's name. Extra is `<team> » <name team>`.
    ///
    /// The team is the one whose address equals the Maintainer's address
    /// exactly, case-sensitive: `team+pkg-go@tracker.debian.org` (golang),
    /// `debian-clojure@lists.debian.org` (clojure),
    /// `pkg-java-maintainers@lists.alioth.debian.org` (java),
    /// `pkg-javascript-maintainers@lists.alioth.debian.org` (javascript),
    /// `pkg-perl-maintainers@lists.alioth.debian.org` (perl), and
    /// `team+python@tracker.debian.org` (python). Nothing is reported when the
    /// name contains the team's own name as a whole word, or contains no other
    /// team's name. When several other team names match, the first in the list
    /// above is used. lintian takes them in hash order, so that choice is ours.
    ///
    /// Source package: the Dsc `Maintainer` and `Source`. Source tree: the
    /// source paragraph of `debian/control`. The Maintainer must be a single
    /// mailbox (`Name <user@host>` or bare); anything else is skipped. Location
    /// is empty.
    WrongTeam,
    code = "LN1053",
    tag = "wrong-team",
    default_selected = true,
    experimental = false,
}

/// Maintainer address and team, in lintian's order.
const TEAMS: [(&str, &str); 6] = [
    ("team+pkg-go@tracker.debian.org", "golang"),
    ("debian-clojure@lists.debian.org", "clojure"),
    ("pkg-java-maintainers@lists.alioth.debian.org", "java"),
    (
        "pkg-javascript-maintainers@lists.alioth.debian.org",
        "javascript",
    ),
    ("pkg-perl-maintainers@lists.alioth.debian.org", "perl"),
    ("team+python@tracker.debian.org", "python"),
];

impl SourceRule for WrongTeam {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let (maintainer, source_name) = {
            let fields = match ctx.dsc() {
                Some(dsc) => dsc.fields(),
                None => match ctx.debian_control() {
                    Some(control) => control.source(),
                    None => return,
                },
            };
            (
                fields.unfolded_value("Maintainer").unwrap_or_default(),
                fields.unfolded_value("Source").unwrap_or_default(),
            )
        };
        if maintainer.is_empty() {
            return;
        }
        let Some(mailbox) = parse_mailbox(&maintainer) else {
            return;
        };
        let address = format!("{}@{}", mailbox.user, mailbox.host);
        let Some(&(_, team)) = TEAMS.iter().find(|(known, _)| *known == address) else {
            return;
        };
        if name_contains(&source_name, team) {
            return;
        }
        let mut other_teams: Vec<&str> = Vec::new();
        for (_, other) in TEAMS {
            if other != team && !other_teams.contains(&other) {
                other_teams.push(other);
            }
        }
        let Some(suggested) = other_teams
            .into_iter()
            .find(|other| name_contains(&source_name, other))
        else {
            return;
        };
        ctx.diagnostic(Diagnostic::warn(format!("{team} \u{00bb} {suggested}")));
    }
}

/// Lintian's `name_contains`: the team name as a whole word of the name.
fn name_contains(name: &str, team: &str) -> bool {
    Regex::new(&format!(r"\b{}\b", regex::escape(team)))
        .expect("escaped team name is a valid regex")
        .is_match(name)
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const JAVA: &str = "Debian Java Maintainers <pkg-java-maintainers@lists.alioth.debian.org>";

    fn control(source: &str, maintainer: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: {source}\nMaintainer: {maintainer}\n\
                 Standards-Version: 4.7.2\nSection: devel\n\
                 \nPackage: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    fn dsc(source: &str, maintainer: &str) -> Dsc {
        Dsc::parse(&format!("Source: {source}\nMaintainer: {maintainer}\n")).expect("Dsc fixture")
    }

    #[test]
    fn source_tree() {
        let pass = vec![
            Vec::<(&str, TestFile)>::new(),
            control("libfoo-java", JAVA),
            control("libfoo", JAVA),
            control("java-clojure", JAVA),
            control("clojure-package", "Someone <someone@example.com>"),
        ];
        let fail = vec![
            control("clojure-package", JAVA),
            control("libfoo-clojure", JAVA),
            control("golang-foo", "Foo <debian-clojure@lists.debian.org>"),
        ];
        SourceTreeTester::new(WrongTeam, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let bare = Vec::<(&str, TestFile)>::new();
        let pass = vec![
            SourcePackageCase::from(bare.clone()).dsc(dsc("libfoo-java", JAVA)),
            SourcePackageCase::from(bare.clone()).dsc(dsc("libfoo", "Foo <foo@example.com>")),
            SourcePackageCase::from(bare.clone())
                .dsc(dsc("libfoo-clojure", "Foo <foo@example.com>")),
            SourcePackageCase::from(bare.clone()).dsc(dsc("libfoo", JAVA)),
        ];
        let fail = vec![
            SourcePackageCase::from(bare.clone()).dsc(dsc("clojure-package", JAVA)),
            SourcePackageCase::from(bare.clone()).dsc(dsc(
                "clojure-package",
                "Java <pkg-java-maintainers@lists.alioth.debian.org>",
            )),
            SourcePackageCase::from(bare)
                .dsc(dsc("perl-foo", "Foo <team+python@tracker.debian.org>")),
        ];
        SourcePackageTester::new(WrongTeam, pass, fail).test_and_snapshot();
    }
}
