//! Rules for lintian's `files/config-scripts` Check.

mod ln0734_old_style_config_script_multiarch_path;

pub(crate) use ln0734_old_style_config_script_multiarch_path::OldStyleConfigScriptMultiarchPath;
mod ln0735_old_style_config_script_multiarch_path_arch_all;

pub(crate) use ln0735_old_style_config_script_multiarch_path_arch_all::OldStyleConfigScriptMultiarchPathArchAll;
