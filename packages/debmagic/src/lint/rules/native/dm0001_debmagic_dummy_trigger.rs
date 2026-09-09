use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::SourceRule;

pub struct DebmagicDummyTrigger;

declare_rule! {
    /// Dummy Rule that fires when `debian/debmagic-dummy-lint` exists in the
    /// packaging tree (Source tree or Source package Patched files).
    ///
    /// Used to exercise Selection, reporting, and FailOn without a lintian Tag.
    DebmagicDummyTrigger,
    code = "DM0001",
    tag = "debmagic-dummy-trigger",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DebmagicDummyTrigger {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let present = ctx.files().is_some_and(|files| {
            files
                .get("debian/debmagic-dummy-lint")
                .is_some_and(|entry| {
                    matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
                })
        });
        if !present {
            return;
        }

        ctx.diagnostic(
            Diagnostic::warn("dummy Rule triggered").with_location(Location {
                path: PathBuf::from("debian/debmagic-dummy-lint"),
                line: None,
                column: None,
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::SourceTreeTester;

    use super::*;

    #[test]
    fn test() {
        let pass = vec![vec![("debian/changelog", "")]];
        let fail = vec![vec![
            ("debian/control", "Source: example\n"),
            ("debian/debmagic-dummy-lint", "trigger\n"),
        ]];
        SourceTreeTester::new(DebmagicDummyTrigger, pass, fail).test_and_snapshot();
    }
}
