//! Rules for lintian's `lintian` Check: the reports on override files (ADR 0057).

mod ln1779_unused_override;
mod ln1780_mismatched_override;

pub(crate) use ln1779_unused_override::UnusedOverride;
pub(crate) use ln1780_mismatched_override::MismatchedOverride;
