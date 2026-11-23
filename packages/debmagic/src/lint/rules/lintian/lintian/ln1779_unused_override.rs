use crate::declare_rule;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::override_pass::OverrideContext;
use crate::lint::rule::{OverrideRule, Severity};

pub struct UnusedOverride;

declare_rule! {
    /// A kept Lintian override names a Tag that a Rule ran for on this
    /// processable, but that Rule made no Diagnostic of that Tag, so the
    /// override silenced nothing. An override whose Tag was renamed is reported
    /// under the name it is written with. Extra is the Tag as written, a space
    /// and the context pattern, untrimmed. Location is the override file and the
    /// override's line. Info. Not Selected by default: lintian enables an info
    /// Tag whatever its display level, so `--select unused-override` reports
    /// them. Applies to the Source tree, Source package and Binary package. An
    /// override of a Tag no Rule ran for, of an architecture the processable is
    /// not, or of a Show-Always Tag is not reported (ADR 0057).
    UnusedOverride,
    code = "LN1779",
    tag = "unused-override",
    default_selected = false,
    experimental = false,
}

impl OverrideRule for UnusedOverride {
    fn run(&self, ctx: &mut OverrideContext<'_>) {
        for unmatched in ctx.unused().to_vec() {
            let location = ctx.location(&unmatched);
            let extra = format!("{} {}", unmatched.tag_as_written, unmatched.pattern);
            ctx.diagnostic(
                Diagnostic::warn(extra)
                    .with_severity(Severity::Info)
                    .with_location(location),
            );
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
            // Matched: the Tag produced a Diagnostic the override silenced.
            OverrideCase::new("cute-field HOMEPAGE*\n")
                .ran(&["cute-field"])
                .diagnostic(cute("HOMEPAGE vs Homepage")),
            // The Tag's Rule did not run here: the override takes no part.
            OverrideCase::new("cute-field\n"),
            // Another architecture: the override is dropped, not unused.
            OverrideCase::new("[amd64]: cute-field\n").ran(&["cute-field"]),
            // Show-Always Tags cannot be overridden, so their overrides are not unused.
            OverrideCase::new("malformed-override\n").ran(&["malformed-override"]),
            // A Tag that produced a Diagnostic that the context missed is mismatched, not unused.
            OverrideCase::new("cute-field nope\n")
                .ran(&["cute-field"])
                .diagnostic(cute("X-Dh-Compat vs X-DH-Compat")),
        ];
        let fail = vec![
            OverrideCase::new("cute-field\n").ran(&["cute-field"]),
            // An old name is reported as written, with the Rule of the current name.
            OverrideCase::new("package-install-apt-sources\n")
                .ran(&["package-installs-apt-sources"]),
            // A Binary package: its own override file, the same rule.
            OverrideCase::new("cute-field\n")
                .binary("amd64")
                .ran(&["cute-field"]),
        ];
        OverrideTester::new(UnusedOverride, pass, fail).test();
    }
}
