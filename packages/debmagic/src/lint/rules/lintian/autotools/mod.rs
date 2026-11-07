//! Rules for lintian's autotools Checks.
//!
//! Each Rule owns one Tag. `libtool` reads patched `ltconfig` and `ltmain.sh`
//! and the source Build-Depends fields.

mod ln0379_autotools_pkg_config_macro_not_cross_compilation_safe;

pub(crate) use ln0379_autotools_pkg_config_macro_not_cross_compilation_safe::AutotoolsPkgConfigMacroNotCrossCompilationSafe;
