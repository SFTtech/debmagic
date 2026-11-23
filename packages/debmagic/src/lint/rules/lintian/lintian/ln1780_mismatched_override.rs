use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::override_pass::OverrideContext;
use crate::lint::rule::OverrideRule;

pub struct MismatchedOverride;

declare_rule! {
    /// A kept Lintian override names a Tag that produced at least one Diagnostic
    /// on this processable, but none of them matched the override's context, so
    /// the override did not silence what it was written for. Whether or not
    /// another override silenced the Diagnostics does not matter. Extra is the
    /// Tag as written, a space and the context pattern, untrimmed. Location is
    /// the override file and the override's line. Warning, default Selection, as
    /// lintian. Applies to the Source tree, Source package and Binary package.
    /// Rules with partial coverage of a Tag can report a mismatch lintian would
    /// not (ADR 0057).
    MismatchedOverride,
    code = "LN1780",
    tag = "mismatched-override",
    default_selected = true,
    experimental = false,
}

impl OverrideRule for MismatchedOverride {
    fn run(&self, ctx: &mut OverrideContext<'_>) {
        for unmatched in ctx.mismatched().to_vec() {
            let location = ctx.location(&unmatched);
            let extra = format!("{} {}", unmatched.tag_as_written, unmatched.pattern);
            ctx.diagnostic(Diagnostic::warn(extra).with_location(location));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::diagnostic::Diagnostic;
    use crate::lint::rule::Tag;
    use crate::lint::tester::{OverrideCase, OverrideTester};

    use super::*;

    fn cute(message: &str) -> Diagnostic {
        Diagnostic::warn(message).with_tag(Tag("cute-field"))
    }

    #[test]
    fn test() {
        let pass = vec![
            // Matched: nothing is mismatched.
            OverrideCase::new("cute-field HOMEPAGE*\n")
                .ran(&["cute-field"])
                .diagnostic(cute("HOMEPAGE vs Homepage")),
            // The Tag produced nothing: that is unused-override, not mismatched.
            OverrideCase::new("cute-field nope\n").ran(&["cute-field"]),
            // The Tag's Rule did not run here: the override takes no part.
            OverrideCase::new("cute-field nope\n").diagnostic(cute("X-Dh-Compat vs X-DH-Compat")),
            // Another architecture: the override is dropped, not mismatched.
            OverrideCase::new("[amd64]: cute-field nope\n")
                .ran(&["cute-field"])
                .diagnostic(cute("X-Dh-Compat vs X-DH-Compat")),
            // A Show-Always Tag cannot be overridden, so it is never mismatched.
            OverrideCase::new("malformed-override nope\n")
                .ran(&["malformed-override"])
                .diagnostic(Diagnostic::warn("x").with_tag(Tag("malformed-override"))),
        ];
        let fail = vec![
            // The Tag fired on the processable, the context does not match.
            OverrideCase::new("cute-field nope\n")
                .ran(&["cute-field"])
                .diagnostic(cute("HOMEPAGE vs Homepage")),
            // An override silenced one Diagnostic but another did not match: the
            // mismatch is reported for the override that matched nothing.
            OverrideCase::new("cute-field HOMEPAGE*\ncute-field X-Dh*\n")
                .ran(&["cute-field"])
                .diagnostic(cute("HOMEPAGE vs Homepage"))
                .diagnostic(cute("Xs-Python vs XS-Python")),
            // A binary package: its own override file, the same rule.
            OverrideCase::new("cute-field nope\n")
                .binary("amd64")
                .ran(&["cute-field"])
                .diagnostic(cute("HOMEPAGE vs Homepage")),
        ];
        OverrideTester::new(MismatchedOverride, pass, fail).test();
    }
}
