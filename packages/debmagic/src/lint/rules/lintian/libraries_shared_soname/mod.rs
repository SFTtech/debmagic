//! `libraries/shared/soname`: a shared object in a library directory without a SONAME.

mod ln0716_sharedobject_in_library_directory_missing_soname;
mod ln1128_package_name_doesnt_match_sonames;

pub(crate) use ln0716_sharedobject_in_library_directory_missing_soname::SharedobjectInLibraryDirectoryMissingSoname;
pub(crate) use ln1128_package_name_doesnt_match_sonames::PackageNameDoesntMatchSonames;
