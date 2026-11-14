//! Rules for lintian's `files/artifact` Check, the non-native `debian-adds-` Tags.
//!
//! `added` is the shared walk. The first matching Perl pattern wins, so one
//! path emits one Tag.

mod added;
mod ln0136_debian_adds_arch_control_dir;
mod ln0137_debian_adds_arch_inventory_file;
mod ln0138_debian_adds_bts_control_dir;
mod ln0139_debian_adds_bzr_control_dir;
mod ln0140_debian_adds_cvs_conflict_copy;
mod ln0141_debian_adds_cvs_control_dir;
mod ln0142_debian_adds_editor_backup_file;
mod ln0143_debian_adds_git_control_dir;
mod ln0144_debian_adds_hg_control_dir;
mod ln0145_debian_adds_hg_tags_file;
mod ln0146_debian_adds_patch_failure_file;
mod ln0147_debian_adds_svk_commit_file;
mod ln0148_debian_adds_svn_commit_file;
mod ln0149_debian_adds_svn_conflict_file;
mod ln0150_debian_adds_svn_control_dir;
mod ln0151_debian_adds_quilt_control_dir;
#[cfg(test)]
mod testing;

pub(crate) use ln0136_debian_adds_arch_control_dir::DebianAddsArchControlDir;
pub(crate) use ln0137_debian_adds_arch_inventory_file::DebianAddsArchInventoryFile;
pub(crate) use ln0138_debian_adds_bts_control_dir::DebianAddsBtsControlDir;
pub(crate) use ln0139_debian_adds_bzr_control_dir::DebianAddsBzrControlDir;
pub(crate) use ln0140_debian_adds_cvs_conflict_copy::DebianAddsCvsConflictCopy;
pub(crate) use ln0141_debian_adds_cvs_control_dir::DebianAddsCvsControlDir;
pub(crate) use ln0142_debian_adds_editor_backup_file::DebianAddsEditorBackupFile;
pub(crate) use ln0143_debian_adds_git_control_dir::DebianAddsGitControlDir;
pub(crate) use ln0144_debian_adds_hg_control_dir::DebianAddsHgControlDir;
pub(crate) use ln0145_debian_adds_hg_tags_file::DebianAddsHgTagsFile;
pub(crate) use ln0146_debian_adds_patch_failure_file::DebianAddsPatchFailureFile;
pub(crate) use ln0147_debian_adds_svk_commit_file::DebianAddsSvkCommitFile;
pub(crate) use ln0148_debian_adds_svn_commit_file::DebianAddsSvnCommitFile;
pub(crate) use ln0149_debian_adds_svn_conflict_file::DebianAddsSvnConflictFile;
pub(crate) use ln0150_debian_adds_svn_control_dir::DebianAddsSvnControlDir;
pub(crate) use ln0151_debian_adds_quilt_control_dir::DebianAddsQuiltControlDir;
