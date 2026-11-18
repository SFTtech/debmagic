//! Rules for lintian's `build-systems/libtool/la-file` Check.

mod ln0769_incorrect_libdir_in_la_file;
mod ln0770_non_empty_dependency_libs_in_la_file;

pub(crate) use ln0769_incorrect_libdir_in_la_file::IncorrectLibdirInLaFile;
pub(crate) use ln0770_non_empty_dependency_libs_in_la_file::NonEmptyDependencyLibsInLaFile;
