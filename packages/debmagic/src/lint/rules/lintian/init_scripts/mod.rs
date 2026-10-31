//! Init script and systemd unit Rules.
//!
//! `init_script` reads `etc/init.d` script text. `systemd_unit` reads unit
//! files. Each Rule owns one Tag.

mod init_script;
mod ln0230_init_d_script_does_not_implement_required_option;
mod ln0231_init_d_script_stops_in_s_runlevel;
mod ln0232_init_d_script_missing_lsb_section;
mod ln0233_systemd_service_file_refers_to_obsolete_target;
mod ln0234_missing_systemd_service_for_init_d_rcs_script;
mod ln0235_systemd_service_file_wraps_init_script;
mod systemd_unit;

pub(crate) use ln0230_init_d_script_does_not_implement_required_option::InitDScriptDoesNotImplementRequiredOption;
pub(crate) use ln0231_init_d_script_stops_in_s_runlevel::InitDScriptStopsInSRunlevel;
pub(crate) use ln0232_init_d_script_missing_lsb_section::InitDScriptMissingLsbSection;
pub(crate) use ln0233_systemd_service_file_refers_to_obsolete_target::SystemdServiceFileRefersToObsoleteTarget;
pub(crate) use ln0234_missing_systemd_service_for_init_d_rcs_script::MissingSystemdServiceForInitDRcSScript;
pub(crate) use ln0235_systemd_service_file_wraps_init_script::SystemdServiceFileWrapsInitScript;
