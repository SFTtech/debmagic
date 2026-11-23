//! Rules for lintian's `files/permissions` Check.

mod ln0700_non_standard_executable_perm;
mod ln0942_executable_is_not_world_readable;
mod ln0943_non_standard_setuid_executable_perm;
mod ln0944_bad_perm_for_file_in_etc_sudoers_d;
mod ln0945_bad_permissions_for_ali_file;
mod ln0946_non_standard_dir_perm;
mod ln0947_non_standard_file_perm;
mod ln0997_elevated_privileges;
mod ln1723_executable_in_usr_lib;
mod ln1792_non_standard_game_executable_perm;
mod owner_exceptions;

pub(crate) use ln0700_non_standard_executable_perm::NonStandardExecutablePerm;
pub(crate) use ln0942_executable_is_not_world_readable::ExecutableIsNotWorldReadable;
pub(crate) use ln0943_non_standard_setuid_executable_perm::NonStandardSetuidExecutablePerm;
pub(crate) use ln0944_bad_perm_for_file_in_etc_sudoers_d::BadPermForFileInEtcSudoersD;
pub(crate) use ln0945_bad_permissions_for_ali_file::BadPermissionsForAliFile;
pub(crate) use ln0946_non_standard_dir_perm::NonStandardDirPerm;
pub(crate) use ln0947_non_standard_file_perm::NonStandardFilePerm;
pub(crate) use ln0997_elevated_privileges::ElevatedPrivileges;
pub(crate) use ln1723_executable_in_usr_lib::ExecutableInUsrLib;
pub(crate) use ln1792_non_standard_game_executable_perm::NonStandardGameExecutablePerm;
