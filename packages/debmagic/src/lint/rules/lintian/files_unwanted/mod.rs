//! Rules for lintian's `files/unwanted` Check.

mod ln0789_backup_file_in_package;
mod ln0790_nfs_temporary_file_in_package;

pub(crate) use ln0789_backup_file_in_package::BackupFileInPackage;
pub(crate) use ln0790_nfs_temporary_file_in_package::NfsTemporaryFileInPackage;
