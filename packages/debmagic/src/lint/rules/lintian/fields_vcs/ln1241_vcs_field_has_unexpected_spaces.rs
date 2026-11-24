use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::super::vcs::PLATFORMS;

pub struct VcsFieldHasUnexpectedSpaces;

declare_rule! {
    /// A `Vcs-*` URI has whitespace that its platform's splitter does not
    /// absorb, for example `Vcs-Git: <url> --branch wrong`.
    ///
    /// Reads `Vcs-Browser`, `-Arch`, `-Bzr`, `-Cvs`, `-Darcs`, `-Hg`, `-Git`,
    /// `-Svn`, and `-Mtn` from the Dsc, or the source paragraph when there is
    /// no Dsc. The URI is split like lintian does (a trailing `[subdir]`, `-b
    /// branch`, or module name is not the URI); whitespace left in the first
    /// part is flagged. Extra is the platform and the unfolded field. Location
    /// is empty.
    VcsFieldHasUnexpectedSpaces,
    code = "LN1241",
    tag = "vcs-field-has-unexpected-spaces",
    default_selected = true,
    experimental = false,
}

static CVS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+?)(?:\s+(\S*))?$").expect("valid regex"));
static HG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+?)(?:\s+-b\s+(\S*))?$").expect("valid regex"));
static GIT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(.+?)(?:(?:\s+\[(\S*)\])?(?:\s+-b\s+(\S*))?){0,2}$").expect("valid regex")
});
static MTN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+?)(?:\s+\S+)?$").expect("valid regex"));

/// The first part of lintian's `%VCS_EXTRACT` split of a `Vcs-<platform>`
/// value, or `None` when the splitter yields nothing. Every other part is
/// `\S*` in lintian's splitters, so whitespace can only occur in the first.
pub(super) fn first_part(platform: &str, uri: &str) -> Option<String> {
    let regex = match platform {
        "Cvs" => &*CVS,
        "Hg" => &*HG,
        "Git" => &*GIT,
        "Mtn" => &*MTN,
        _ => return Some(uri.to_string()),
    };
    regex
        .captures(uri)
        .and_then(|captures| captures.get(1))
        .map(|first| first.as_str().to_string())
}

impl SourceRule for VcsFieldHasUnexpectedSpaces {
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
        let Some(first) = first_part(platform, &uri) else {
            continue;
        };
        // Perl truthiness: an empty first part, or "0", is not a URI.
        if first.is_empty() || first == "0" {
            continue;
        }
        if first.chars().any(char::is_whitespace) {
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
            control("Vcs-Git: https://salsa.debian.org/debian/foo.git\n"),
            control("Vcs-Git: https://salsa.debian.org/debian/foo.git -b main\n"),
            control("Vcs-Git: https://salsa.debian.org/debian/foo.git [sub] -b main\n"),
            control("Vcs-Hg: https://example.com/hg/foo -b default\n"),
            control("Vcs-Cvs: :pserver:anoncvs@example.com:/cvs mod\n"),
            control(""),
        ];
        let fail = vec![
            control("Vcs-Git: https://anonscm.debian.org/test/test.git --branch wrong\n"),
            control("Vcs-Browser: https://example.com/a b\n"),
            control("Vcs-Svn: https://example.com/svn/a b\n"),
        ];
        SourceTreeTester::new(VcsFieldHasUnexpectedSpaces, pass, fail).test_and_snapshot();
    }
}
