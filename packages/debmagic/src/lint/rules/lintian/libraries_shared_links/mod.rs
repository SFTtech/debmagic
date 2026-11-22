//! Rules for lintian's `libraries/shared/links` Check.

mod ln1410_lacks_versioned_link_to_shared_library;
mod shared;

pub(crate) use ln1410_lacks_versioned_link_to_shared_library::LacksVersionedLinkToSharedLibrary;
