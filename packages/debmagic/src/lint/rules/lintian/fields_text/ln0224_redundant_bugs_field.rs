use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

pub struct RedundantBugsField;

declare_rule! {
    /// The `Bugs` field repeats the Debian default.
    ///
    /// Reads Installation control. The unfolded value must match
    /// `debbugs://bugs.debian.org` with an optional trailing slash, any case
    /// (the dots are any character, as in lintian). A `-dbgsym` package is not
    /// exempt. Extra and Location are empty. Udebs are checked too.
    RedundantBugsField,
    code = "LN0224",
    tag = "redundant-bugs-field",
    default_selected = true,
    experimental = false,
}

static DEFAULT_BUGS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^debbugs://bugs.debian.org/?$").expect("valid regex"));

impl BinaryPackageRule for RedundantBugsField {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let redundant = ctx
            .installation_control()
            .fields()
            .unfolded_value("Bugs")
            .is_some_and(|bugs| DEFAULT_BUGS.is_match(&bugs));
        if redundant {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn bugs(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Bugs", value)
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            bugs("https://bugs.debian.org/"),
            bugs("debbugs://bugs.debian.org/foo"),
            bugs("debbugs://bugs.debian.org//"),
            bugs("mailto:debian-backports@lists.debian.org"),
        ];
        let fail = vec![
            bugs("debbugs://bugs.debian.org/"),
            bugs("debbugs://bugs.debian.org"),
            bugs("DEBBUGS://BUGS.DEBIAN.ORG/"),
            bugs("debbugs://bugsXdebianYorg"),
            bugs("debbugs://bugs.debian.org/").package("example-dbgsym"),
            bugs("debbugs://bugs.debian.org/").udeb(),
        ];
        BinaryPackageTester::new(RedundantBugsField, pass, fail).test_and_snapshot();
    }
}
