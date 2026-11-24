//! Rules for lintian's `maintainer-scripts/killall` Check.
//!
//! Each Rule owns one Tag and reads the Binary package's Control files.

mod ln0811_killall_is_dangerous;

pub(crate) use ln0811_killall_is_dangerous::KillallIsDangerous;
