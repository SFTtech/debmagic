use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::{ControlParagraph, unfold};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::vcs::PLATFORMS;

pub struct VcsFieldUsesNotRecommendedUriFormat;

declare_rule! {
    /// A `Vcs-*` URI is valid for its platform and not the recommended form.
    ///
    /// Reads `Vcs-Browser`, `-Arch`, `-Bzr`, `-Cvs`, `-Darcs`, `-Hg`, `-Git`,
    /// `-Svn`, and `-Mtn` from the Dsc, or the source paragraph when there is
    /// no Dsc. `git@host:path` and `svn+ssh://` are the usual cases. Extra is
    /// the platform and the unfolded field. Location is empty.
    VcsFieldUsesNotRecommendedUriFormat,
    code = "LN0135",
    tag = "vcs-field-uses-not-recommended-uri-format",
    default_selected = true,
    experimental = false,
}

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

static CVS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+?)(?:\s+(\S*))?$").expect("valid regex"));
static HG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+?)(?:\s+-b\s+(\S*))?$").expect("valid regex"));
static GIT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(.+?)(?:(?:\s+\[(\S*)\])?(?:\s+-b\s+(\S*))?){0,2}$").expect("valid regex")
});
static MTN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+?)(?:\s+\S+)?$").expect("valid regex"));

impl SourceRule for VcsFieldUsesNotRecommendedUriFormat {
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
        let name = format!("Vcs-{platform}");
        let Some(uri) = fields.get(&name).map(unfold) else {
            continue;
        };
        if uri.is_empty() {
            continue;
        }
        let Some(head) = first_part(platform, &uri) else {
            continue;
        };
        if head.is_empty() {
            continue;
        }
        let Some(recommended) = regex_for(&RECOMMENDED, platform) else {
            continue;
        };
        if recommended.is_match(&head) {
            continue;
        }
        let Some(valid) = regex_for(&VALID, platform) else {
            continue;
        };
        if valid.is_match(&head) {
            found.push(format!("{platform} {uri}"));
        }
    }
    found
}

fn regex_for<'a>(table: &'a [(&str, Regex)], platform: &str) -> Option<&'a Regex> {
    table
        .iter()
        .find(|(name, _)| *name == platform)
        .map(|(_, regex)| regex)
}

fn first_part(platform: &str, uri: &str) -> Option<String> {
    let regex = match platform {
        "Cvs" => Some(&*CVS),
        "Hg" => Some(&*HG),
        "Git" => Some(&*GIT),
        "Mtn" => Some(&*MTN),
        _ => None,
    };
    match regex {
        Some(regex) => regex
            .captures(uri)
            .and_then(|captures| captures.get(1))
            .map(|part| part.as_str().to_string()),
        None => Some(uri.to_string()),
    }
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
    fn test() {
        let pass = vec![
            control("Vcs-Git: https://salsa.debian.org/foo/bar.git\n"),
            control("Vcs-Browser: https://example.com/\n"),
        ];
        let fail = vec![
            control("Vcs-Git: git@git.debian.org:foobar.git\n"),
            control("Vcs-Svn: svn+ssh://svn.debian.org/svn/foobar/trunk\n"),
            control("Vcs-Mtn: www.example.org org.debian.foobar\n"),
        ];
        SourceTreeTester::new(VcsFieldUsesNotRecommendedUriFormat, pass, fail).test_and_snapshot();
    }
}
