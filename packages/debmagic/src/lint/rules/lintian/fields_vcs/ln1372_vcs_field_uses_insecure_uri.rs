use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::{ControlParagraph, unfold};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::super::vcs::PLATFORMS;

pub struct VcsFieldUsesInsecureUri;

declare_rule! {
    /// A `Vcs-*` URI uses an unencrypted transport: `git://`, `http://`,
    /// `nosmart+http://`, `svn://`, or Bazaar's `lp:` and CVS's `:pserver:`.
    ///
    /// Reads the same fields as `vcs-field-uses-not-recommended-uri-format`,
    /// from the Dsc or the source paragraph of `debian/control`. The URI is
    /// split with lintian's `%VCS_EXTRACT` pattern for the platform (Cvs,
    /// Hg, Git and Mtn drop their trailing options) and the head is tested.
    /// Extra is the platform and the unfolded field. Location is empty.
    /// Severity is info and the Tag is not selected by default, as in lintian.
    VcsFieldUsesInsecureUri,
    code = "LN1372",
    tag = "vcs-field-uses-insecure-uri",
    default_selected = false,
    experimental = false,
}

static INSECURE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:git|(?:nosmart\+)?http|svn)://|^(?:lp|:pserver):").expect("valid regex")
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

impl SourceRule for VcsFieldUsesInsecureUri {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = match ctx.dsc() {
            Some(dsc) => messages(dsc.fields()),
            None => ctx
                .debian_control()
                .map(|control| messages(control.source()))
                .unwrap_or_default(),
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::warn(message).with_severity(Severity::Info));
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
        if INSECURE.is_match(&head) {
            found.push(format!("{platform} {uri}"));
        }
    }
    found
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
            control("Vcs-Svn: svn+ssh://svn.debian.org/svn/foobar/trunk\n"),
            control("Vcs-Cvs: :ext:anonymous@cvs.example.org:/cvs foobar\n"),
        ];
        let fail = vec![
            control("Vcs-Git: git://salsa.debian.org/users/toddy/foobar.git\n"),
            control("Vcs-Browser: http://salsa.debian.org/toddy/foobar\n"),
            control("Vcs-Svn: svn://anonscm.debian.org/foobar/trunk\n"),
            control("Vcs-Cvs: :pserver:anonymous@cvs.example.org:/cvsroot/foobar module\n"),
            control("Vcs-Bzr: lp:foobar\n"),
            control("Vcs-Hg: nosmart+http://hg.example.org/foobar\n"),
        ];
        SourceTreeTester::new(VcsFieldUsesInsecureUri, pass, fail).test_and_snapshot();
    }
}
