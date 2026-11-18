//! Rules for lintian's `usrmerge` Check.
//!
//! Root-level entries whose `usr/` twin is installed by the same package.

mod ln0780_file_in_root_and_usr;
mod ln0781_library_in_root_and_usr;
mod shadow;

pub(crate) use ln0780_file_in_root_and_usr::FileInRootAndUsr;
pub(crate) use ln0781_library_in_root_and_usr::LibraryInRootAndUsr;
