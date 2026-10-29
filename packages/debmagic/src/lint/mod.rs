mod binary_package_reader;
mod context;
mod debhelper;
mod debian_changelog;
mod debian_control;
mod debian_copyright;
mod diagnostic;
mod dsc;
mod file_tree;
mod file_type;
mod installation_control;
mod intent;
mod package_relation;
mod registry;
mod rule;
mod rules;
mod run;
mod selection;
mod source_package;
mod subject;
#[cfg(test)]
pub(crate) mod tester;

pub use diagnostic::{Diagnostic, DiagnosticInner, Location, ProcessableType};
pub use intent::{LintIntent, LintIntentInput, resolve_lint_intent};
pub use rule::{Code, Severity, Tag};
pub use run::{
    LintOutcome, LintResult, format_diagnostic, format_finished, format_summary, run_lint,
};
pub use subject::{Subject, SubjectKind};
