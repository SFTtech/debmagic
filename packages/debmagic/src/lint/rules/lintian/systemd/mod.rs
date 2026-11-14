//! Rules for lintian's `systemd` Check.
//!
//! `systemd_init` reads `etc/init.d` scripts. Unit-file Rules read service
//! text through `init_scripts::systemd_unit`. Each Rule owns one Tag.

mod ln0125_missing_systemd_service_for_init_d_script;
mod ln0126_init_d_script_does_not_source_init_functions;
mod ln0295_systemd_service_alias_without_extension;
mod ln0296_systemd_service_file_refers_to_obsolete_bindto;
mod ln0297_systemd_service_file_uses_nobody_or_nogroup;
mod ln0412_missing_systemd_timer_for_cron_script;
mod systemd_init;
mod targets;

pub(crate) use ln0125_missing_systemd_service_for_init_d_script::MissingSystemdServiceForInitDScript;
pub(crate) use ln0126_init_d_script_does_not_source_init_functions::InitDScriptDoesNotSourceInitFunctions;
pub(crate) use ln0295_systemd_service_alias_without_extension::SystemdServiceAliasWithoutExtension;
pub(crate) use ln0296_systemd_service_file_refers_to_obsolete_bindto::SystemdServiceFileRefersToObsoleteBindto;
pub(crate) use ln0297_systemd_service_file_uses_nobody_or_nogroup::SystemdServiceFileUsesNobodyOrNogroup;
pub(crate) use ln0412_missing_systemd_timer_for_cron_script::MissingSystemdTimerForCronScript;
