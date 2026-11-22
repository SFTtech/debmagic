use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};

pub struct BadIntendedDistribution;

declare_rule! {
    /// The newest installed changelog entry says it is for one distribution
    /// (`Upload to experimental`, `uploads to testing`, ...) but its heading
    /// names another. The extra is `intended for <dist> but uploaded to <dist>`
    /// and the Location is the heading line of the entry.
    ///
    /// The intended distribution is the first `upload(s) to` phrase of the
    /// entry's text, case-insensitively; `sid` counts as `unstable` on both
    /// sides. An entry whose heading says `UNRELEASED` (in capitals only, as
    /// lintian compares) is not reported. Info severity, experimental and not
    /// selected by default, as in lintian.
    ///
    /// Read from the installed changelog, as lintian's `binary` part of the
    /// `debian/changelog` Check does; udebs are not left out.
    BadIntendedDistribution,
    code = "LN1515",
    tag = "bad-intended-distribution",
    default_selected = false,
    experimental = true,
}

static INTENDED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)uploads?\s+to\s+(testing|unstable|experimental|sid)").expect("valid regex")
});

impl BinaryPackageRule for BadIntendedDistribution {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(installed) = ctx.installed_changelog() else {
            return;
        };
        let path: PathBuf = installed.path.clone();
        let Some(latest) = installed.parsed.entries().first() else {
            return;
        };
        let Some(captures) = INTENDED.captures(latest.changes()) else {
            return;
        };
        let mut intended = captures[1].to_ascii_lowercase();
        if intended == "sid" {
            intended = "unstable".to_string();
        }
        let mut uploaded = latest.distribution().to_string();
        if uploaded == "sid" {
            uploaded = "unstable".to_string();
        }
        if uploaded == intended || uploaded == "UNRELEASED" {
            return;
        }
        ctx.diagnostic(
            Diagnostic::warn(format!(
                "intended for {intended} but uploaded to {uploaded}"
            ))
            .with_severity(Severity::Info)
            .with_location(Location {
                path,
                line: Some(latest.line()),
                column: None,
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn entry(distribution: &str, changes: &str) -> String {
        format!(
            "example (1.0) {distribution}; urgency=low\n\n  * {changes}\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    fn packaged(text: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            "usr/share/doc/example/changelog".to_string(),
            TestFile::new(text.to_string()),
        )])
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            // Intended and uploaded agree, and sid is unstable.
            packaged(&entry("unstable", "Upload to unstable.")),
            packaged(&entry("unstable", "Uploads to sid.")),
            packaged(&entry("sid", "Upload to unstable.")),
            // UNRELEASED is not reported, in capitals.
            packaged(&entry("UNRELEASED", "Upload to experimental.")),
            // No phrase, or the phrase is not about a distribution.
            packaged(&entry("unstable", "Fix the upload to the server.")),
            // Only the newest entry is looked at.
            packaged(&format!(
                "{}\n{}",
                entry("unstable", "New release."),
                entry("experimental", "Upload to experimental.")
            )),
        ];
        let fail = vec![
            packaged(&entry("unstable", "Upload to experimental.")),
            packaged(&entry("experimental", "Uploads to sid.")),
            // The phrase is case-insensitive, but UNRELEASED is not special
            // in lower case.
            packaged(&entry("unreleased", "UPLOAD TO testing.")),
        ];
        BinaryPackageTester::new(BadIntendedDistribution, pass, fail).test_and_snapshot();
    }
}
