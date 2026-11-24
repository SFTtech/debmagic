use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::super::vcs::PLATFORMS;
use super::ln1241_vcs_field_has_unexpected_spaces::first_part;

pub struct VcsFieldUsesUnknownUriFormat;

declare_rule! {
    /// A `Vcs-*` URI matches no known format for its platform.
    ///
    /// Reads `Vcs-Browser`, `-Arch`, `-Bzr`, `-Cvs`, `-Darcs`, `-Hg`, `-Git`,
    /// `-Svn`, and `-Mtn` from the Dsc, or the source paragraph when there is
    /// no Dsc. Flagged when the splitter yields no first part, when that part
    /// does not start with the platform's recommended scheme and is not in its
    /// valid-but-not-recommended set (that case is
    /// `vcs-field-uses-not-recommended-uri-format`). Extra is the platform and
    /// the unfolded field. Location is empty.
    VcsFieldUsesUnknownUriFormat,
    code = "LN1242",
    tag = "vcs-field-uses-unknown-uri-format",
    default_selected = true,
    experimental = false,
}

// Lintian's `%VCS_RECOMMENDED_URIS` and `%VCS_VALID_URIS`. A copy of the
// tables in the LN0135 Rule, which keeps them private to its module.
static RECOMMENDED: LazyLock<Vec<(&str, Regex)>> = LazyLock::new(|| {
    vec![
        ("Browser", Regex::new(r"^https?://").expect("valid regex")),
        ("Arch", Regex::new(r"^https?://").expect("valid regex")),
        (
            "Bzr",
            Regex::new(r"^(?:lp:|(?:nosmart\+)?https?://)").expect("valid regex"),
        ),
        (
            "Cvs",
            Regex::new(r"^:(?:pserver:|ext:_?anoncvs)").expect("valid regex"),
        ),
        ("Darcs", Regex::new(r"^https?://").expect("valid regex")),
        ("Hg", Regex::new(r"^https?://").expect("valid regex")),
        (
            "Git",
            Regex::new(r"^(?:git|https?|rsync)://").expect("valid regex"),
        ),
        (
            "Svn",
            Regex::new(r"^(?:svn|(?:svn\+)?https?)://").expect("valid regex"),
        ),
        ("Mtn", Regex::new(r"^mtn://").expect("valid regex")),
    ]
});

static VALID: LazyLock<Vec<(&str, Regex)>> = LazyLock::new(|| {
    vec![
        ("Arch", Regex::new(r"^https?://").expect("valid regex")),
        (
            "Bzr",
            Regex::new(r"^(?:sftp|(?:bzr\+)?ssh)://").expect("valid regex"),
        ),
        (
            "Cvs",
            Regex::new(r"^(?:-d\s*)?:(?:ext|pserver):").expect("valid regex"),
        ),
        ("Hg", Regex::new(r"^ssh://").expect("valid regex")),
        (
            "Git",
            Regex::new(r"^(?:git\+)?ssh://|^[\w.]+@[a-zA-Z0-9.]+:[/a-zA-Z0-9.]")
                .expect("valid regex"),
        ),
        (
            "Svn",
            Regex::new(r"^(?:svn\+)?ssh://").expect("valid regex"),
        ),
        ("Mtn", Regex::new(r"^[\w.-]+$").expect("valid regex")),
    ]
});

fn pattern<'a>(table: &'a [(&str, Regex)], platform: &str) -> Option<&'a Regex> {
    table
        .iter()
        .find(|(name, _)| *name == platform)
        .map(|(_, regex)| regex)
}

impl SourceRule for VcsFieldUsesUnknownUriFormat {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = match ctx.dsc() {
            Some(dsc) => messages(dsc.fields()),
            None => ctx
                .debian_control()
                .map(|control| messages(control.source()))
                .unwrap_or_default(),
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message));
        }
    }
}

fn messages(fields: &ControlParagraph) -> Vec<String> {
    let mut found = Vec::new();
    for platform in PLATFORMS {
        let Some(uri) = fields.unfolded_value(&format!("Vcs-{platform}")) else {
            continue;
        };
        let first = match first_part(platform, &uri) {
            // Perl truthiness: an empty first part, or "0", is not a URI.
            Some(first) if !first.is_empty() && first != "0" => first,
            _ => {
                found.push(format!("{platform} {uri}"));
                continue;
            }
        };
        let Some(recommended) = pattern(&RECOMMENDED, platform) else {
            continue;
        };
        if recommended.is_match(&first) {
            continue;
        }
        let valid = pattern(&VALID, platform).is_some_and(|valid| valid.is_match(&first));
        if !valid {
            found.push(format!("{platform} {uri}"));
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestFile};

    use super::*;

    fn control(fields: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: Jane <jane@example.com>\n\
                 Standards-Version: 4.7.2\n{fields}\n\
                 Package: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn source() {
        let pass = vec![
            control("Vcs-Browser: https://salsa.debian.org/debian/foo\n"),
            control("Vcs-Git: https://salsa.debian.org/debian/foo.git\n"),
            control("Vcs-Git: git@salsa.debian.org:debian/foo.git\n"),
            control("Vcs-Bzr: lp:foo\n"),
            control("Vcs-Svn: svn+ssh://example.com/svn/foo\n"),
            control(""),
        ];
        let fail = vec![
            control("Vcs-Browser: svn.debian.org/wsvn/foobar/trunk\n"),
            control("Vcs-Git: ftp://example.com/foo\n"),
            control("Vcs-Hg: ftp://example.com/hg/foo\n"),
        ];
        SourceTreeTester::new(VcsFieldUsesUnknownUriFormat, pass, fail).test_and_snapshot();
    }
}
