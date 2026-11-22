use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::ln1241_vcs_field_has_unexpected_spaces::first_part;

pub struct VcsGitUsesInvalidUserUri;

declare_rule! {
    /// A `Vcs-Git` URI uses the `git://` user form `~user` on a Debian host.
    ///
    /// Reads `Vcs-Git` from the Dsc, or the source paragraph when there is no
    /// Dsc. Flagged when the first part of the URI starts with
    /// `git://git.debian.org/~` or `git://anonscm.debian.org/~`. Extra is the
    /// platform, the URI as written, and the URI canonicalized the way lintian
    /// does (`https://anonscm.debian.org/git/users/...`). Location is empty.
    VcsGitUsesInvalidUserUri,
    code = "LN1243",
    tag = "vcs-git-uses-invalid-user-uri",
    default_selected = true,
    experimental = false,
}

static INVALID_USER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"git://(?:git|anonscm)\.debian\.org/~").expect("valid regex"));

// The rest of lintian's `%VCS_CANONIFY` for Git, applied in order after the
// user-URI rewrite. Each replaces the first match only, as Perl's `s{}{}` does.
static CANONIFY: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    vec![
        (
            Regex::new(r"git://(?:git|anonscm)\.debian\.org/~").expect("valid regex"),
            "https://anonscm.debian.org/git/users/",
        ),
        (
            Regex::new(r"(https?://.*?\.git)(?:\.git)+$").expect("valid regex"),
            "$1",
        ),
        (
            Regex::new(r"https?://git\.debian\.org/(?:git/?)?").expect("valid regex"),
            "https://anonscm.debian.org/git/",
        ),
        (
            Regex::new(r"https?://anonscm\.debian\.org/git/git/").expect("valid regex"),
            "https://anonscm.debian.org/git/",
        ),
        (
            Regex::new(r"git://git\.debian\.org/(?:git/?)?").expect("valid regex"),
            "https://anonscm.debian.org/git/",
        ),
        (
            Regex::new(r"git://anonscm\.debian\.org/git/").expect("valid regex"),
            "https://anonscm.debian.org/git/",
        ),
        (
            Regex::new(r"https?://salsa\.debian\.org/([^/]+/[^/.]+)$").expect("valid regex"),
            "https://salsa.debian.org/$1.git",
        ),
    ]
});

impl SourceRule for VcsGitUsesInvalidUserUri {
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
    let Some(uri) = fields.unfolded_value("Vcs-Git") else {
        return Vec::new();
    };
    let Some(first) = first_part("Git", &uri) else {
        return Vec::new();
    };
    if !INVALID_USER.is_match(&first) {
        return Vec::new();
    }
    let mut canonicalized = first.clone();
    for (regex, replacement) in CANONIFY.iter() {
        canonicalized = regex.replace(&canonicalized, *replacement).into_owned();
    }
    vec![format!("Git {first} {canonicalized}")]
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
            control("Vcs-Git: https://anonscm.debian.org/git/users/djpig/foobar.git\n"),
            control("Vcs-Git: git://git.debian.org/git/foobar.git\n"),
            control("Vcs-Git: git://example.com/~djpig/foobar.git\n"),
            control("Vcs-Browser: git://git.debian.org/~djpig/foobar.git\n"),
            control(""),
        ];
        let fail = vec![
            control("Vcs-Git: git://git.debian.org/~djpig/foobar.git\n"),
            control("Vcs-Git: git://anonscm.debian.org/~djpig/foobar.git -b main\n"),
        ];
        SourceTreeTester::new(VcsGitUsesInvalidUserUri, pass, fail).test_and_snapshot();
    }
}
