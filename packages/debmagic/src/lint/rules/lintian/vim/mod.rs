//! Rules for lintian's `vim` Check.

mod ln1147_vim_addon_within_vim_runtime_path;

pub(crate) use ln1147_vim_addon_within_vim_runtime_path::VimAddonWithinVimRuntimePath;
