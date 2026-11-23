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
mod ln0926_no_debian_changes;
mod ln1244_source_contains_editor_backup_file;
mod ln1245_source_contains_patch_failure_file;
mod ln1600_source_contains_arch_control_dir;
mod ln1601_source_contains_arch_inventory_file;
mod ln1602_source_contains_bts_control_dir;
mod ln1603_source_contains_bzr_control_dir;
mod ln1604_source_contains_cvs_conflict_copy;
mod ln1605_source_contains_cvs_control_dir;
mod ln1606_source_contains_git_control_dir;
mod ln1607_source_contains_hg_control_dir;
mod ln1608_source_contains_hg_tags_file;
mod ln1609_source_contains_svk_commit_file;
mod ln1610_source_contains_svn_commit_file;
mod ln1611_source_contains_svn_conflict_file;
mod ln1612_source_contains_svn_control_dir;
mod ln1772_source_contains_quilt_control_dir;
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
pub(crate) use ln0926_no_debian_changes::NoDebianChanges;
pub(crate) use ln1244_source_contains_editor_backup_file::SourceContainsEditorBackupFile;
pub(crate) use ln1245_source_contains_patch_failure_file::SourceContainsPatchFailureFile;
pub(crate) use ln1600_source_contains_arch_control_dir::SourceContainsArchControlDir;
pub(crate) use ln1601_source_contains_arch_inventory_file::SourceContainsArchInventoryFile;
pub(crate) use ln1602_source_contains_bts_control_dir::SourceContainsBtsControlDir;
pub(crate) use ln1603_source_contains_bzr_control_dir::SourceContainsBzrControlDir;
pub(crate) use ln1604_source_contains_cvs_conflict_copy::SourceContainsCvsConflictCopy;
pub(crate) use ln1605_source_contains_cvs_control_dir::SourceContainsCvsControlDir;
pub(crate) use ln1606_source_contains_git_control_dir::SourceContainsGitControlDir;
pub(crate) use ln1607_source_contains_hg_control_dir::SourceContainsHgControlDir;
pub(crate) use ln1608_source_contains_hg_tags_file::SourceContainsHgTagsFile;
pub(crate) use ln1609_source_contains_svk_commit_file::SourceContainsSvkCommitFile;
pub(crate) use ln1610_source_contains_svn_commit_file::SourceContainsSvnCommitFile;
pub(crate) use ln1611_source_contains_svn_conflict_file::SourceContainsSvnConflictFile;
pub(crate) use ln1612_source_contains_svn_control_dir::SourceContainsSvnControlDir;
pub(crate) use ln1772_source_contains_quilt_control_dir::SourceContainsQuiltControlDir;
