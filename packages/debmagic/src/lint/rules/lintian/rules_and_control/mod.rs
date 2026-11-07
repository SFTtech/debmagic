//! Rules for lintian's `debhelper`, `debug/automatic`, and
//! `debian/control/field/misplaced` Checks that read `debian/rules` or a
//! control field.
//!
//! Build-dependency names and the `debian/rules` line reader are shared with
//! [`super::debian_rules`].

mod control_text;
mod ln0217_debhelper_tools_from_autotools_dev_are_deprecated;
mod ln0220_brace_expansion_in_debhelper_config_file;
mod ln0221_package_uses_dh_exec_but_lacks_build_depends;
mod ln0222_debian_control_has_dbgsym_package;
mod ln0223_build_prerequisite_in_installable_section;
mod ln0419_debian_control_repeats_field_name_in_value;
mod rules_text;

pub(super) use control_text::{build_depends_all_names, names_package};
pub(super) use rules_text::{read_rules, rules_lines};

pub(crate) use ln0217_debhelper_tools_from_autotools_dev_are_deprecated::DebhelperToolsFromAutotoolsDevAreDeprecated;
pub(crate) use ln0220_brace_expansion_in_debhelper_config_file::BraceExpansionInDebhelperConfigFile;
pub(crate) use ln0221_package_uses_dh_exec_but_lacks_build_depends::PackageUsesDhExecButLacksBuildDepends;
pub(crate) use ln0222_debian_control_has_dbgsym_package::DebianControlHasDbgsymPackage;
pub(crate) use ln0223_build_prerequisite_in_installable_section::BuildPrerequisiteInInstallableSection;
pub(crate) use ln0419_debian_control_repeats_field_name_in_value::DebianControlRepeatsFieldNameInValue;
