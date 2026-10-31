//! Rules for lintian's `debian/rules` Check.
//!
//! Each Rule reads `debian/rules` with comments dropped and `\` continuations
//! joined, and owns one Tag. [`text`] is that reader. Rules that also need a
//! build-dependency name read it from `rules_and_control`.

mod ln0003_debian_rules_missing_required_target;
mod ln0016_custom_compression_in_debian_rules;
mod ln0017_debian_rules_ignores_make_clean_error;
mod ln0179_debian_rules_calls_pwd;
mod ln0180_debian_rules_calls_debhelper_in_odd_order;
mod ln0182_binary_arch_rules_but_pkg_is_arch_indep;
mod ln0216_debian_rules_uses_supported_python_versions_without_python_all_build_depends;
mod ln0218_debian_rules_calls_nproc;
mod ln0219_override_dh_fixperms_does_not_call_dh_fixperms;
mod text;

pub(super) use text::{complete_lines, read_rules_from_tree};

pub(crate) use ln0003_debian_rules_missing_required_target::DebianRulesMissingRequiredTarget;
pub(crate) use ln0016_custom_compression_in_debian_rules::CustomCompressionInDebianRules;
pub(crate) use ln0017_debian_rules_ignores_make_clean_error::DebianRulesIgnoresMakeCleanError;
pub(crate) use ln0179_debian_rules_calls_pwd::DebianRulesCallsPwd;
pub(crate) use ln0180_debian_rules_calls_debhelper_in_odd_order::DebianRulesCallsDebhelperInOddOrder;
pub(crate) use ln0182_binary_arch_rules_but_pkg_is_arch_indep::BinaryArchRulesButPkgIsArchIndep;
pub(crate) use ln0216_debian_rules_uses_supported_python_versions_without_python_all_build_depends::DebianRulesUsesSupportedPythonVersionsWithoutPythonAllBuildDepends;
pub(crate) use ln0218_debian_rules_calls_nproc::DebianRulesCallsNproc;
pub(crate) use ln0219_override_dh_fixperms_does_not_call_dh_fixperms::OverrideDhFixpermsDoesNotCallDhFixperms;
