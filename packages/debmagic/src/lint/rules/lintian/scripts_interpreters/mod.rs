//! Rules for lintian's `scripts` Check that read installed scripts and
//! maintainer scripts for their interpreter.

mod ln0704_missing_dep_for_interpreter;
mod ln0711_unusual_interpreter;
mod ln0715_script_without_interpreter;

pub(crate) use ln0704_missing_dep_for_interpreter::MissingDepForInterpreter;
pub(crate) use ln0715_script_without_interpreter::ScriptWithoutInterpreter;

mod ln0718_wrong_path_for_interpreter;
mod ln0719_php_script_but_no_php_cli_dep;

pub(crate) use ln0711_unusual_interpreter::UnusualInterpreter;
pub(crate) use ln0718_wrong_path_for_interpreter::WrongPathForInterpreter;
pub(crate) use ln0719_php_script_but_no_php_cli_dep::PhpScriptButNoPhpCliDep;
