//! Rules that flag a Data files path under a forbidden prefix.
//!
//! Each Rule owns the predicate for its Tag. `hierarchy_standard` is only
//! the order of lintian's `files/hierarchy/standard` chain, so an earlier
//! branch still hides a later one. `dir-or-file-in-srv` and
//! `dir-or-file-in-mnt` are prefix checks outside that chain (`flag`).

mod flag;
mod hierarchy_standard;
mod ln0035_dir_or_file_in_srv;
mod ln0036_dir_or_file_in_mnt;
mod ln0038_dir_or_file_in_home;
mod ln0039_dir_or_file_in_tmp;
mod ln0040_dir_or_file_in_opt;
mod ln0041_dir_or_file_in_var_www;
mod ln0042_dir_or_file_in_var_run;
mod ln0043_dir_or_file_in_var_lock;
mod ln0044_dir_or_file_in_run;
mod ln0045_dir_or_file_in_etc_opt;
mod ln0046_dir_in_usr_local;
mod ln0047_file_in_usr_local;
mod ln0048_file_directly_in_usr_share;
mod ln0049_subdir_in_usr_bin;

#[cfg(test)]
mod tree;

pub(crate) use ln0035_dir_or_file_in_srv::DirOrFileInSrv;
pub(crate) use ln0036_dir_or_file_in_mnt::DirOrFileInMnt;
pub(crate) use ln0038_dir_or_file_in_home::DirOrFileInHome;
pub(crate) use ln0039_dir_or_file_in_tmp::DirOrFileInTmp;
pub(crate) use ln0040_dir_or_file_in_opt::DirOrFileInOpt;
pub(crate) use ln0041_dir_or_file_in_var_www::DirOrFileInVarWww;
pub(crate) use ln0042_dir_or_file_in_var_run::DirOrFileInVarRun;
pub(crate) use ln0043_dir_or_file_in_var_lock::DirOrFileInVarLock;
pub(crate) use ln0044_dir_or_file_in_run::DirOrFileInRun;
pub(crate) use ln0045_dir_or_file_in_etc_opt::DirOrFileInEtcOpt;
pub(crate) use ln0046_dir_in_usr_local::DirInUsrLocal;
pub(crate) use ln0047_file_in_usr_local::FileInUsrLocal;
pub(crate) use ln0048_file_directly_in_usr_share::FileDirectlyInUsrShare;
pub(crate) use ln0049_subdir_in_usr_bin::SubdirInUsrBin;
