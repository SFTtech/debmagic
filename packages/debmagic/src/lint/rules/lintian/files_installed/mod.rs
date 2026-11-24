//! Installed-path Rules whose Check is a filename or directory predicate.
//!
//! `files/sgml`, `desktop/gnome`, and `cron`. Each Rule owns one Tag. Directory
//! index names are the stored path; these patterns do not add a trailing slash.

mod ln0394_file_in_usr_lib_sgml;
mod ln0400_package_installs_into_etc_gconf_schemas;
mod ln0406_run_parts_cron_filename_contains_illegal_chars;
mod ln0859_bad_permissions_for_etc_cron_d_script;

pub(crate) use ln0394_file_in_usr_lib_sgml::FileInUsrLibSgml;
pub(crate) use ln0400_package_installs_into_etc_gconf_schemas::PackageInstallsIntoEtcGconfSchemas;
pub(crate) use ln0406_run_parts_cron_filename_contains_illegal_chars::RunPartsCronFilenameContainsIllegalChars;
pub(crate) use ln0859_bad_permissions_for_etc_cron_d_script::BadPermissionsForEtcCronDScript;
