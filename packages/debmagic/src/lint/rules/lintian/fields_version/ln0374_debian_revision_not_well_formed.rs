use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

use super::source_version;

pub struct DebianRevisionNotWellFormed;

declare_rule! {
    /// The Debian revision has more than three dotted parts, or a bad start.
    ///
    /// The revision is what follows the last hyphen (`0` when there is none).
    /// It is well formed when it reads `X`, `X.Y`, or `X.Y.Z` with non-empty
    /// parts. A revision with a further dotted part is not, unless it is an
    /// Ubuntu revision (`...ubuntuN` followed by one to three `.N` parts) that
    /// the extra part follows. Reads the Dsc when there is one, otherwise the
    /// newest changelog heading on a Source tree, and Installation control on a
    /// Binary package. Extra is the Version.
    DebianRevisionNotWellFormed,
    code = "LN0374",
    tag = "debian-revision-not-well-formed",
    default_selected = true,
    experimental = false,
}

static WELL_FORMED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:[^.]+)(?:\.[^.]+)?(?:\.[^.]+)?(\..*)?$").expect("valid regex")
});

static UBUNTU: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:[^.]+ubuntu[^.]+)(?:\.[0-9]+){1,3}(\..*)?$").expect("valid regex")
});

impl SourceRule for DebianRevisionNotWellFormed {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(version) = source_version(ctx) else {
            return;
        };
        if !revision_well_formed(&version_revision(&version)) {
            ctx.diagnostic(Diagnostic::warn(version));
        }
    }
}

impl BinaryPackageRule for DebianRevisionNotWellFormed {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(version) = ctx
            .installation_control()
            .fields()
            .unfolded_value("Version")
        else {
            return;
        };
        if !revision_well_formed(&version_revision(&version)) {
            ctx.diagnostic(Diagnostic::warn(version));
        }
    }
}

/// The text after the last hyphen, or `0` when the version has none.
fn version_revision(version: &str) -> String {
    match version.rfind('-') {
        Some(index) => version[index + 1..].to_string(),
        None => "0".to_string(),
    }
}

fn revision_well_formed(revision: &str) -> bool {
    let Some(caps) = WELL_FORMED.captures(revision) else {
        return false;
    };
    if caps.get(1).is_none() {
        return true;
    }
    UBUNTU
        .captures(revision)
        .is_some_and(|ubuntu| ubuntu.get(1).is_none())
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, SourceTreeTester, TestFile};

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

    fn version(value: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field("Version", value)
    }

    #[test]
    fn source_tree() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree("1.0-1"),
            tree("1.0"),
            tree("1.0-1.1"),
            tree("1.0-1.2.3"),
            tree("1.0-1ubuntu0.5.10.1"),
        ];
        let fail = vec![tree("0-.3"), tree("1.5-.3"), tree("1.0-1.2.3.4")];
        SourceTreeTester::new(DebianRevisionNotWellFormed, pass, fail).test_and_snapshot();
    }

    #[test]
    fn binary_package() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            version("1.0-1"),
            version("1.0-1ubuntu1.2"),
        ];
        let fail = vec![version("1.5-.3"), version("1.0-1.2.3.4")];
        BinaryPackageTester::new(DebianRevisionNotWellFormed, pass, fail)
            .test_and_snapshot_with_suffix("binary");
    }
}
