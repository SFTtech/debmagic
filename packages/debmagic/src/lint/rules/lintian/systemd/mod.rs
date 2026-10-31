//! Rules for lintian's `systemd` Check that read `etc/init.d` scripts.
//!
//! `systemd_init` finds those scripts and whether a unit is shipped. Each Rule
//! owns one Tag.

mod ln0125_missing_systemd_service_for_init_d_script;
mod ln0126_init_d_script_does_not_source_init_functions;
mod systemd_init;

pub(crate) use ln0125_missing_systemd_service_for_init_d_script::MissingSystemdServiceForInitDScript;
pub(crate) use ln0126_init_d_script_does_not_source_init_functions::InitDScriptDoesNotSourceInitFunctions;
