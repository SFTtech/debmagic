use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debhelper::experimental_compat_level;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

pub struct PackageUsesExperimentalDebhelperCompatVersion;

declare_rule! {
    /// The debhelper compat level is at or above lintian's experimental
    /// threshold (`data/debhelper/compat-level`, `experimental`): the level is
    /// marked experimental by the debhelper developer, so a stable level is
    /// preferred.
    ///
    /// Emitted under the same conditions as `package-uses-old-debhelper-compat-version`
    /// (see that Rule): `debian/rules` shows a dh command and the declared
    /// level comes from `debian/compat`, `Build-Depends: debhelper-compat (= N)`
    /// or `X-DH-Compat`; nothing declared is level 1 and never reaches the
    /// threshold. Extra is the level. Pedantic and not selected by default, as
    /// in lintian. Applies to the Source tree and Source package.
    ///
    /// The threshold is 15 in the vendored data, and debhelper rejects a
    /// level of 15 or more, so the Tag is only reached with a lower threshold:
    /// the Parity recipe runs with an `--include-dir` that sets it to 10.
    PackageUsesExperimentalDebhelperCompatVersion,
    code = "LN1681",
    tag = "package-uses-experimental-debhelper-compat-version",
    default_selected = false,
    experimental = false,
}

impl SourceRule for PackageUsesExperimentalDebhelperCompatVersion {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = ctx.debhelper() else {
            return;
        };
        if !facts.uses_debhelper() {
            return;
        }
        let level = facts.level();
        if is_experimental(level, experimental_compat_level()) {
            ctx.diagnostic(Diagnostic::warn(level.to_string()).with_severity(Severity::Pedantic));
        }
    }
}

/// Lintian's `$debhelper_level >= $DEBHELPER_LEVELS->value('experimental')`.
fn is_experimental(level: u32, threshold: u64) -> bool {
    u64::from(level) >= threshold
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourceTreeTester, TestCase};

    use super::*;

    #[test]
    fn threshold_is_inclusive() {
        assert!(!is_experimental(9, 10));
        assert!(is_experimental(10, 10));
        assert!(is_experimental(13, 10));
        assert!(!is_experimental(13, 15));
    }

    #[test]
    fn source() {
        // The vendored threshold is 15, which debhelper rejects, so no
        // level that reaches the Rule emits it here (see the Parity recipe).
        let pass = Vec::<TestCase>::new();
        SourceTreeTester::new(
            PackageUsesExperimentalDebhelperCompatVersion,
            pass,
            Vec::new(),
        )
        .test();
    }
}
