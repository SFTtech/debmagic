mod context;
mod debian_control;
mod diagnostic;
mod intent;
mod registry;
mod rule;
mod rules;
mod run;
mod selection;
mod subject;
#[cfg(test)]
pub(crate) mod tester;

pub use diagnostic::{Diagnostic, DiagnosticInner, Location};
pub use intent::{CheckIntent, CheckIntentInput, resolve_check_intent};
pub use rule::{Code, Severity, Tag};
pub use run::{CheckOutcome, format_diagnostic, format_summary, run_check};
pub use subject::{Subject, SubjectKind};
