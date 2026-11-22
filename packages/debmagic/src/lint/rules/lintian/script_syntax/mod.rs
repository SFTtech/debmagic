//! Rules for lintian's `script/syntax` Check.

mod ln1425_maintainer_shell_script_fails_syntax_check;
mod ln1426_shell_script_fails_syntax_check;
mod syntax;

pub(crate) use ln1425_maintainer_shell_script_fails_syntax_check::MaintainerShellScriptFailsSyntaxCheck;
pub(crate) use ln1426_shell_script_fails_syntax_check::ShellScriptFailsSyntaxCheck;
