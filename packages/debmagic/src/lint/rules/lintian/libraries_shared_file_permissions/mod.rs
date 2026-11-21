//! Rules for lintian's `libraries/shared/file-permissions` Check.

mod ln1125_odd_permissions_on_shared_library;
mod ln1126_shared_library_is_executable;

pub(crate) use ln1125_odd_permissions_on_shared_library::OddPermissionsOnSharedLibrary;
pub(crate) use ln1126_shared_library_is_executable::SharedLibraryIsExecutable;
