//! Rules for lintian's `maintainer-scripts/systemctl` Check.
//!
//! Each Rule owns one Tag and reads the Binary package's Control files.

mod ln0809_maintainer_script_calls_systemctl;

pub(crate) use ln0809_maintainer_script_calls_systemctl::MaintainerScriptCallsSystemctl;
