mod binary_package_reader;
mod context;
mod debian_changelog;
mod debian_control;
mod debian_rules;
mod diagnostic;
mod installation_control;
mod intent;
mod registry;
mod rule;
mod rules;
mod run;
mod selection;
mod subject;
#[cfg(test)]
pub(crate) mod tester;

pub use diagnostic::{Diagnostic, DiagnosticInner, Location, ProcessableType};
pub use intent::{CheckIntent, CheckIntentInput, resolve_check_intent};
pub use rule::{Code, Severity, Tag};
pub use run::{CheckOutcome, format_diagnostic, format_summary, run_check};
pub use subject::{Subject, SubjectKind};
