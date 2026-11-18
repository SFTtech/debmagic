use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::{ControlParagraph, unfold};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct VcsFieldBitrotted;

declare_rule! {
    /// A `Vcs-Browser` or `Vcs-Cvs` URI points at a retired location.
    ///
    /// Reads `Vcs-Browser` and `Vcs-Cvs` from the Dsc, or the source paragraph
    /// when there is no Dsc. A CVS root with `/cvsroot/` on
    /// `cvs.alioth.debian.org` or `anonscm.debian.org` is rewritten to
    /// `/cvs/`. A ViewVC `op=log` browser link (on `anonscm.debian.org`, or
    /// `svn.debian.org/wsvn/` first rewritten there) loses its `op=log`
    /// parameter. Extra is the platform, the original value (for Cvs, the root
    /// without the module), and the canonical value. Location is empty.
    VcsFieldBitrotted,
    code = "LN0797",
    tag = "vcs-field-bitrotted",
    default_selected = true,
    experimental = false,
}

static CVS_SPLIT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+?)(?:\s+(\S*))?$").expect("valid regex"));
static CVS_ROOT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"@(?:cvs\.alioth|anonscm)\.debian\.org:/cvsroot/").expect("valid regex")
});
static WSVN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https?://svn\.debian\.org/wsvn/").expect("valid regex"));
static VIEWVC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https?://anonscm\.debian\.org/viewvc/").expect("valid regex"));
static OP_LOG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\?(.*[;&])?op=log(?:[;&](.*))?$").expect("valid regex"));

impl SourceRule for VcsFieldBitrotted {
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
    if let Some(uri) = fields.get("Vcs-Browser").map(unfold)
        && let Some(canonical) = browser(&uri)
    {
        found.push(format!("Browser {uri} {canonical}"));
    }
    if let Some(uri) = fields.get("Vcs-Cvs").map(unfold) {
        let root = CVS_SPLIT
            .captures(&uri)
            .and_then(|captures| captures.get(1))
            .map_or("", |part| part.as_str());
        if let Some(canonical) = cvs(root) {
            found.push(format!("Cvs {root} {canonical}"));
        }
    }
    found
}

/// The canonical form when the ViewVC `op=log` rewrite applies.
fn browser(uri: &str) -> Option<String> {
    let rewritten = WSVN.replace(uri, "https://anonscm.debian.org/viewvc/");
    if !VIEWVC.is_match(&rewritten) {
        return None;
    }
    let captures = OP_LOG.captures(&rewritten)?;
    let whole = captures.get(0)?;
    let mut canonical = rewritten[..whole.start()].to_string();
    let kept: String = [captures.get(1), captures.get(2)]
        .into_iter()
        .flatten()
        .map(|part| part.as_str())
        .collect();
    if !kept.is_empty() {
        canonical.push('?');
        canonical.push_str(&kept);
    }
    Some(canonical)
}

/// The canonical form when the CVS root is retired.
fn cvs(root: &str) -> Option<String> {
    if !CVS_ROOT.is_match(root) {
        return None;
    }
    let rewritten = CVS_ROOT.replace(root, "@anonscm.debian.org:/cvs/");
    Some(rewritten.replacen(
        "@cvs.alioth.debian.org:/cvs/",
        "@anonscm.debian.org:/cvs/",
        1,
    ))
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
            control("Vcs-Browser: https://salsa.debian.org/foo/bar\n"),
            control("Vcs-Browser: https://anonscm.debian.org/viewvc/foo/trunk?rev=3\n"),
            control("Vcs-Cvs: :pserver:anon@anonscm.debian.org:/cvs/foo module\n"),
            control("Vcs-Cvs: :pserver:anon@example.org:/cvsroot/foo\n"),
            control("Vcs-Browser: https://svn.debian.org/wsvn/foo/trunk\n"),
        ];
        let fail = vec![
            control("Vcs-Cvs: :pserver:anonymous@cvs.alioth.debian.org:/cvsroot/foobar module\n"),
            control("Vcs-Cvs: :pserver:anon@anonscm.debian.org:/cvsroot/foo\n"),
            control(
                "Vcs-Browser: https://svn.debian.org/wsvn/foobar/trunk?foo=bar;op=log;something=else\n",
            ),
            control("Vcs-Browser: https://anonscm.debian.org/viewvc/foo/trunk?op=log\n"),
        ];
        SourceTreeTester::new(VcsFieldBitrotted, pass, fail).test_and_snapshot();
    }
}
