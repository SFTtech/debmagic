//! Rules for lintian's `maintainer-scripts/adduser` Check.
//!
//! Each Rule owns one Tag and reads the Binary package's Control files.

mod ln0813_adduser_with_home_var_run;

pub(crate) use ln0813_adduser_with_home_var_run::AdduserWithHomeVarRun;
