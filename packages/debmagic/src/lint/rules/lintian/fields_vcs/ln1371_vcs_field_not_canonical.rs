use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

use super::super::vcs::PLATFORMS;
use super::ln1241_vcs_field_has_unexpected_spaces::first_part;

pub struct VcsFieldNotCanonical;

declare_rule! {
    /// A `Vcs-*` URI is an old address for a host that has a current one, such
    /// as `svn://svn.debian.org/...` for `svn://anonscm.debian.org/...`.
    ///
    /// Reads `Vcs-Browser`, `-Arch`, `-Bzr`, `-Cvs`, `-Hg`, `-Git`, and `-Svn`
    /// from the Dsc, or the source paragraph when there is no Dsc. The first
    /// part of the URI (as `%VCS_EXTRACT` splits it) is rewritten with lintian's
    /// substitutions, in its order, each replacing its first match only. The
    /// Tag is reported when the rewrite changes the URI, with extra `<platform>
    /// <uri> <canonical>`. Rewrites that lintian reports under another Tag
    /// (`vcs-field-bitrotted` for a viewvc `op=log` link, a Cvs `cvsroot`
    /// address, `vcs-git-uses-invalid-user-uri` for `git://.../~`) are not
    /// reported here. Location is empty. Info.
    VcsFieldNotCanonical,
    code = "LN1371",
    tag = "vcs-field-not-canonical",
    default_selected = false,
    experimental = false,
}

/// Replaces the first match of `pattern` in `text`, when there is one.
fn replace_first(text: &str, pattern: &str, replacement: &str) -> Option<String> {
    let regex = Regex::new(pattern).expect("valid regex");
    regex
        .is_match(text)
        .then(|| regex.replace(text, replacement).into_owned())
}

/// Applies one rewrite in place, reporting whether it matched.
fn rewrite(text: &mut String, pattern: &str, replacement: &str) -> bool {
    match replace_first(text, pattern, replacement) {
        Some(replaced) => {
            *text = replaced;
            true
        }
        None => false,
    }
}

/// The canonical form of a platform's URI, and whether lintian reports the
/// rewrite under another Tag. `None` for a platform lintian does not
/// canonicalize.
fn canonicalize(platform: &str, uri: &str) -> Option<(String, bool)> {
    let mut text = uri.to_string();
    let mut other_tag = false;
    match platform {
        "Browser" => {
            rewrite(
                &mut text,
                r"https?://svn\.debian\.org/wsvn/",
                "https://anonscm.debian.org/viewvc/",
            );
            rewrite(
                &mut text,
                r"https?://git\.debian\.org/\?p=",
                "https://anonscm.debian.org/git/",
            );
            rewrite(
                &mut text,
                r"https?://bzr\.debian\.org/loggerhead/",
                "https://anonscm.debian.org/loggerhead/",
            );
            rewrite(
                &mut text,
                r"https?://salsa\.debian\.org/([^/]+/[^/]+)\.git/?$",
                "https://salsa.debian.org/$1",
            );
            other_tag = viewvc_op_log(&mut text);
        }
        "Cvs" => {
            other_tag = rewrite(
                &mut text,
                r"@(?:cvs\.alioth|anonscm)\.debian\.org:/cvsroot/",
                "@anonscm.debian.org:/cvs/",
            );
            rewrite(
                &mut text,
                r"@cvs\.alioth\.debian\.org:/cvs/",
                "@anonscm.debian.org:/cvs/",
            );
        }
        "Arch" => {
            rewrite(
                &mut text,
                r"https?://arch\.debian\.org/arch/",
                "https://anonscm.debian.org/arch/",
            );
        }
        "Bzr" => {
            rewrite(
                &mut text,
                r"https?://bzr\.debian\.org/",
                "https://anonscm.debian.org/bzr/",
            );
            rewrite(
                &mut text,
                r"https?://anonscm\.debian\.org/bzr/bzr/",
                "https://anonscm.debian.org/bzr/",
            );
        }
        "Git" => {
            other_tag = rewrite(
                &mut text,
                r"git://(?:git|anonscm)\.debian\.org/~",
                "https://anonscm.debian.org/git/users/",
            );
            rewrite(&mut text, r"(https?://.*?\.git)(?:\.git)+$", "$1");
            rewrite(
                &mut text,
                r"https?://git\.debian\.org/(?:git/?)?",
                "https://anonscm.debian.org/git/",
            );
            rewrite(
                &mut text,
                r"https?://anonscm\.debian\.org/git/git/",
                "https://anonscm.debian.org/git/",
            );
            rewrite(
                &mut text,
                r"git://git\.debian\.org/(?:git/?)?",
                "https://anonscm.debian.org/git/",
            );
            rewrite(
                &mut text,
                r"git://anonscm\.debian\.org/git/",
                "https://anonscm.debian.org/git/",
            );
            rewrite(
                &mut text,
                r"https?://salsa\.debian\.org/([^/]+/[^/.]+)$",
                "https://salsa.debian.org/$1.git",
            );
        }
        "Hg" => {
            rewrite(
                &mut text,
                r"https?://hg\.debian\.org/",
                "https://anonscm.debian.org/hg/",
            );
            rewrite(
                &mut text,
                r"https?://anonscm\.debian\.org/hg/hg/",
                "https://anonscm.debian.org/hg/",
            );
        }
        "Svn" => {
            rewrite(
                &mut text,
                r"svn://cvs\.alioth\.debian\.org/",
                "svn://anonscm.debian.org/",
            );
            rewrite(
                &mut text,
                r"svn://svn\.debian\.org/",
                "svn://anonscm.debian.org/",
            );
            rewrite(
                &mut text,
                r"svn://anonscm\.debian\.org/svn/",
                "svn://anonscm.debian.org/",
            );
        }
        _ => return None,
    }
    Some((text, other_tag))
}

/// Browser `viewvc` links with `op=log`: the query is cut, keeping its other
/// options. Returns whether the cut happened (lintian reports another Tag).
fn viewvc_op_log(text: &mut String) -> bool {
    if !text.contains("https://anonscm.debian.org/viewvc/")
        && !text.contains("http://anonscm.debian.org/viewvc/")
    {
        return false;
    }
    let Ok(regex) = Regex::new(r"(?s)\?(.*[;&])?op=log(?:[;&](.*))?\z") else {
        return false;
    };
    let Some(captures) = regex.captures(text) else {
        return false;
    };
    let whole = captures.get(0).expect("whole match");
    let kept = format!(
        "{}{}",
        captures.get(1).map_or("", |m| m.as_str()),
        captures.get(2).map_or("", |m| m.as_str())
    );
    let mut rewritten = text[..whole.start()].to_string();
    if !kept.is_empty() {
        rewritten.push('?');
        rewritten.push_str(&kept);
    }
    *text = rewritten;
    true
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
        let Some((canonical, other_tag)) = canonicalize(platform, &first) else {
            continue;
        };
        if !other_tag && canonical != first {
            found.push(format!("{platform} {first} {canonical}"));
        }
    }
    found
}

impl SourceRule for VcsFieldNotCanonical {
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

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(fields: &str) -> Vec<(&'static str, TestFile)> {
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: Example <ex@example.com>\n{fields}\nPackage: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn rewrites() {
        assert_eq!(
            canonicalize("Svn", "svn://svn.debian.org/svn/foobar/trunk"),
            Some(("svn://anonscm.debian.org/foobar/trunk".to_string(), false))
        );
        assert_eq!(
            canonicalize("Git", "https://salsa.debian.org/foo/bar.git.git"),
            Some(("https://salsa.debian.org/foo/bar.git".to_string(), false))
        );
        assert_eq!(
            canonicalize("Browser", "https://salsa.debian.org/foo/bar.git/"),
            Some(("https://salsa.debian.org/foo/bar".to_string(), false))
        );
        assert_eq!(
            canonicalize(
                "Browser",
                "https://anonscm.debian.org/viewvc/x?op=log&rev=1"
            ),
            Some((
                "https://anonscm.debian.org/viewvc/x?rev=1".to_string(),
                true
            ))
        );
        assert_eq!(canonicalize("Darcs", "http://example.org/"), None);
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no control"))],
            control("Vcs-Git: https://salsa.debian.org/foo/bar.git\n"),
            control("Vcs-Svn: svn://anonscm.debian.org/foobar/trunk\n"),
            control("Vcs-Darcs: http://example.org/darcs/foo\n"),
        ];
        let fail = vec![
            control("Vcs-Svn: svn://svn.debian.org/svn/foobar/trunk\n"),
            control("Vcs-Git: https://salsa.debian.org/foo/bar\n"),
        ];
        SourceTreeTester::new(VcsFieldNotCanonical, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(control(
            "Vcs-Git: https://salsa.debian.org/foo/bar.git\n",
        ))];
        let fail = vec![SourcePackageCase::from(control(
            "Vcs-Svn: svn://svn.debian.org/svn/foobar/trunk\n",
        ))];
        SourcePackageTester::new(VcsFieldNotCanonical, pass, fail).test_and_snapshot();
    }
}
