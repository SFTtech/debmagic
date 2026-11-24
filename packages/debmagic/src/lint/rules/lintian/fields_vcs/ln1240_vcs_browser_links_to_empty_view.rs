use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::{ControlParagraph, unfold};
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

pub struct VcsBrowserLinksToEmptyView;

declare_rule! {
    /// A `Vcs-Browser` URL links to an empty view of the repository.
    ///
    /// Reads `Vcs-Browser` from the Dsc, or the source paragraph when there is
    /// no Dsc. Flagged when the unfolded URL contains `rev=0&sc=0`, a suffix
    /// that leads to an empty view. Extra is the unfolded URL. Location is
    /// empty.
    VcsBrowserLinksToEmptyView,
    code = "LN1240",
    tag = "vcs-browser-links-to-empty-view",
    default_selected = true,
    experimental = false,
}

impl SourceRule for VcsBrowserLinksToEmptyView {
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
    let Some(uri) = fields.get("Vcs-Browser").map(unfold) else {
        return Vec::new();
    };
    if uri.contains("rev=0&sc=0") {
        vec![uri]
    } else {
        Vec::new()
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
    fn source() {
        let pass = vec![
            control("Vcs-Browser: https://salsa.debian.org/debian/foo\n"),
            control("Vcs-Browser: https://example.com/viewvc/foo/trunk?rev=1\n"),
            control("Vcs-Git: https://example.com/foo.git?rev=0&sc=0\n"),
            control(""),
        ];
        let fail = vec![
            control("Vcs-Browser: https://anonscm.debian.org/viewvc/foobar/trunk?rev=0&sc=0\n"),
            control("Vcs-Browser: https://example.com/foo?rev=0&sc=0&x=1\n"),
        ];
        SourceTreeTester::new(VcsBrowserLinksToEmptyView, pass, fail).test_and_snapshot();
    }
}
