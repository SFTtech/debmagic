//! Rules for lintian's `vim` Check.

mod ln1147_vim_addon_within_vim_runtime_path;
mod ln1746_obsolete_vim_addon_manager;

pub(crate) use ln1147_vim_addon_within_vim_runtime_path::VimAddonWithinVimRuntimePath;
pub(crate) use ln1746_obsolete_vim_addon_manager::ObsoleteVimAddonManager;
