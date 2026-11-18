//! Rules for lintian's `files/multi-arch` Check.

mod ln0721_multiarch_foreign_pkgconfig;
mod ln0765_multiarch_foreign_cmake_file;
mod ln0766_multiarch_foreign_shared_library;

pub(crate) use ln0721_multiarch_foreign_pkgconfig::MultiarchForeignPkgconfig;
pub(crate) use ln0765_multiarch_foreign_cmake_file::MultiarchForeignCmakeFile;
pub(crate) use ln0766_multiarch_foreign_shared_library::MultiarchForeignSharedLibrary;
