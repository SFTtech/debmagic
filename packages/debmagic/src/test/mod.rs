//! Run a package's declared Debian autopkgtest tests.

pub mod intent;
mod run;

pub use intent::{TestIntent, TestIntentInput, resolve_test_intent};
pub use run::{TestOutcome, run_test};
