//! Rules for lintian's autotools Checks.
//!
//! Each Rule owns one Tag. `libtool` reads patched `ltconfig` and `ltmain.sh`
//! and the source Build-Depends fields.

mod ln0379_autotools_pkg_config_macro_not_cross_compilation_safe;
mod ln0387_ancient_libtool;
mod ln0739_configure_generated_file_in_source;

pub(crate) use ln0379_autotools_pkg_config_macro_not_cross_compilation_safe::AutotoolsPkgConfigMacroNotCrossCompilationSafe;
pub(crate) use ln0387_ancient_libtool::AncientLibtool;
pub(crate) use ln0739_configure_generated_file_in_source::ConfigureGeneratedFileInSource;
