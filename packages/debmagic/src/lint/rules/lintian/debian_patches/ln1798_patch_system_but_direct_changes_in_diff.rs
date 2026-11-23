use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::ln1706_direct_changes_in_diff_but_no_patch_system::{
    direct_changes_extra, uses_patch_system,
};

pub struct PatchSystemButDirectChangesInDiff;

declare_rule! {
    /// A patch system is in use (the Format is `3.x (quilt)`, or the
    /// Build-Depends-All satisfy `quilt`) and the Debian diff
    /// (`<name>_<version>.diff.gz`) still changes a file outside `debian/`.
    /// Extra is the first such file in sorted order, with ` and N more` when
    /// there are more; the order differs from lintian's hash order with
    /// several files, as in `direct-changes-in-diff-but-no-patch-system`. The
    /// Location is empty. The file list is [`SourceContext::diffstat`] and the
    /// Rule applies to the Source package only: a Source tree has no diff. The
    /// Tag is warning and selected by default.
    PatchSystemButDirectChangesInDiff,
    code = "LN1798",
    tag = "patch-system-but-direct-changes-in-diff",
    default_selected = true,
    experimental = false,
}

impl SourceRule for PatchSystemButDirectChangesInDiff {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let diffstat = ctx.diffstat();
        let Some(extra) = direct_changes_extra(&diffstat) else {
            return;
        };
        if !uses_patch_system(ctx) {
            return;
        }
        ctx.diagnostic(Diagnostic::warn(extra));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageTester, TestFile};

    use super::super::ln1706_direct_changes_in_diff_but_no_patch_system::tests::{
        CONTROL, CONTROL_QUILT, case, diff_gz,
    };
    use super::*;

    #[test]
    fn source_package() {
        let diff = "example_1.0-1.diff.gz";
        let pass = vec![
            // No patch system: LN1706's business, not this Tag's.
            case(CONTROL, "1.0", diff, diff_gz(&["README"]), true),
            // A patch system, but the diff changes only debian/.
            case(
                CONTROL,
                "3.0 (quilt)",
                diff,
                diff_gz(&["debian/rules"]),
                true,
            ),
            // A patch system, but the diff is not listed in the .dsc.
            case(CONTROL_QUILT, "1.0", diff, diff_gz(&["README"]), false),
            // A patch system with a diff that is not gzip.
            case(CONTROL_QUILT, "1.0", diff, TestFile::new("not gzip"), true),
        ];
        let fail = vec![
            case(CONTROL, "3.0 (quilt)", diff, diff_gz(&["README"]), true),
            // Build-Depends-All satisfy quilt under a 1.0 Format.
            case(
                CONTROL_QUILT,
                "1.0",
                diff,
                diff_gz(&["src/main.c", "README"]),
                true,
            ),
        ];
        SourcePackageTester::new(PatchSystemButDirectChangesInDiff, pass, fail).test_and_snapshot();
    }
}
