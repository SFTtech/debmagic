//! Rules for lintian's `files/permissions` Check.

mod ln0700_non_standard_executable_perm;
mod ln0941_elevated_privileges;
mod ln0942_executable_is_not_world_readable;
mod ln0943_non_standard_setuid_executable_perm;
mod ln0944_bad_perm_for_file_in_etc_sudoers_d;
mod ln0945_bad_permissions_for_ali_file;
mod ln0946_non_standard_dir_perm;
mod ln0947_non_standard_file_perm;

pub(crate) use ln0700_non_standard_executable_perm::NonStandardExecutablePerm;
pub(crate) use ln0941_elevated_privileges::ElevatedPrivileges;
pub(crate) use ln0942_executable_is_not_world_readable::ExecutableIsNotWorldReadable;
pub(crate) use ln0943_non_standard_setuid_executable_perm::NonStandardSetuidExecutablePerm;
pub(crate) use ln0944_bad_perm_for_file_in_etc_sudoers_d::BadPermForFileInEtcSudoersD;
pub(crate) use ln0945_bad_permissions_for_ali_file::BadPermissionsForAliFile;
pub(crate) use ln0946_non_standard_dir_perm::NonStandardDirPerm;
pub(crate) use ln0947_non_standard_file_perm::NonStandardFilePerm;
