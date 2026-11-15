//! Rules for lintian's `scripts` Check that read installed scripts against the
//! interpreter table.

mod ln0704_missing_dep_for_interpreter;

pub(crate) use ln0704_missing_dep_for_interpreter::MissingDepForInterpreter;
