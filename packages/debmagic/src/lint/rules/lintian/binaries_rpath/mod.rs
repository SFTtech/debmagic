//! `binaries/rpath`: RPATH and RUNPATH of an ELF file in a Binary package.

mod ln0717_relative_library_search_path;

pub(crate) use ln0717_relative_library_search_path::RelativeLibrarySearchPath;
