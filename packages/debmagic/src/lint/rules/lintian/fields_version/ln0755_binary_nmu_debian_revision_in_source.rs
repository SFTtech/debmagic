use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::source_version;

pub struct BinaryNmuDebianRevisionInSource;

declare_rule! {
    /// A source package's Version has a binary NMU's revision or `+bN`.
    ///
    /// The revision is what follows the last hyphen (`0` when there is none).
    /// It is flagged when it starts with two dotted parts (`X.Y.`) and is not
    /// an Ubuntu revision whose extra part follows, or when the Version ends
    /// in `+bN`. Reads the Dsc when there is one, otherwise the newest
    /// changelog heading on a Source tree. Extra is the Version.
    BinaryNmuDebianRevisionInSource,
    code = "LN0755",
    tag = "binary-nmu-debian-revision-in-source",
    default_selected = true,
    experimental = false,
}

static TWO_DOTS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[^.-]+\.[^.-]+\.").expect("valid regex"));
static PLUS_BIN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\+b\d+$").expect("valid regex"));
static REVISION_SHAPE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:[^.]+)(?:\.[^.]+)?(?:\.[^.]+)?(\..*)?$").expect("valid regex")
});
static UBUNTU: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:[^.]+ubuntu[^.]+)(?:\.[0-9]+){1,3}(\..*)?$").expect("valid regex")
});

impl SourceRule for BinaryNmuDebianRevisionInSource {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(version) = source_version(ctx) else {
            return;
        };
        let revision = version
            .rsplit_once('-')
            .map_or("0", |(_, revision)| revision);
        if (TWO_DOTS.is_match(revision) && !ubuntu_revision(revision))
            || PLUS_BIN.is_match(&version)
        {
            ctx.diagnostic(Diagnostic::warn(version));
        }
    }
}

/// Perl's `$ubuntu`: the revision has an extra part after the two dotted
/// parts, and it is an Ubuntu revision that the extra part follows.
fn ubuntu_revision(revision: &str) -> bool {
    REVISION_SHAPE
        .captures(revision)
        .is_some_and(|shape| shape.get(1).is_some())
        && UBUNTU.is_match(revision)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn entry(version: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn tree(version: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("debian/control", TestFile::new("Source: example\n")),
            ("debian/changelog", TestFile::new(entry(version))),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree("1.0"),
            tree("1.0-1"),
            tree("1.0-1.1"),
            tree("1.0-1ubuntu1.2"),
            tree("1.0-1ubuntu0.5.10.1"),
            tree("1.0-1~bpo10+1"),
        ];
        let fail = vec![
            tree("1.0-1.0.1"),
            tree("1.0-1.2.3"),
            tree("1.0-1.2.3.4"),
            tree("1.0-1+b1"),
            tree("1.0-1ubuntu1.2.3"),
        ];
        SourceTreeTester::new(BinaryNmuDebianRevisionInSource, pass, fail).test_and_snapshot();
    }
}
