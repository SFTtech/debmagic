//! Rules for lintian's `scripts` Check that read installed scripts and
//! maintainer scripts for their interpreter.

mod ln0704_missing_dep_for_interpreter;
mod ln0715_script_without_interpreter;

pub(crate) use ln0704_missing_dep_for_interpreter::MissingDepForInterpreter;
pub(crate) use ln0715_script_without_interpreter::ScriptWithoutInterpreter;
mod ln0711_unusual_interpreter;

pub(crate) use ln0704_missing_dep_for_interpreter::MissingDepForInterpreter;
pub(crate) use ln0711_unusual_interpreter::UnusualInterpreter;
