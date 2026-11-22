//! Rules for lintian's `libraries/shared/links` Check.

mod ln1410_lacks_versioned_link_to_shared_library;
mod ln1411_ldconfig_symlink_is_not_a_symlink;
mod ln1412_ldconfig_symlink_referencing_wrong_file;
mod shared;

pub(crate) use ln1410_lacks_versioned_link_to_shared_library::LacksVersionedLinkToSharedLibrary;
pub(crate) use ln1411_ldconfig_symlink_is_not_a_symlink::LdconfigSymlinkIsNotASymlink;
pub(crate) use ln1412_ldconfig_symlink_referencing_wrong_file::LdconfigSymlinkReferencingWrongFile;
