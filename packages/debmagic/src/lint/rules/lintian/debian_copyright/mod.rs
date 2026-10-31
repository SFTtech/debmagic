//! Rules for `debian/copyright` and the installed copyright file.
//!
//! `installed_copyright` is what lintian's `debian/copyright` Check reads from
//! `usr/share/doc/<package>/copyright`. Each Rule owns one Tag.

mod installed_copyright;
mod ln0009_no_dep5_copyright;
mod ln0010_no_copyright_file;
mod ln0112_old_fsf_address_in_copyright_file;
mod ln0113_copyright_refers_to_old_directory;
mod ln0114_copyright_refers_to_deprecated_bsd_license_file;
mod ln0115_copyright_refers_to_compressed_license;
mod ln0116_copyright_refers_to_incorrect_directory;
mod ln0117_copyright_refers_to_versionless_license_file;
mod ln0118_copyright_refers_to_nonexistent_license_file;
mod ln0119_copyright_file_compressed;
mod ln0120_copyright_file_is_symlink;
mod ln0121_space_in_std_shortname_in_dep5_copyright;

pub(crate) use ln0009_no_dep5_copyright::NoDep5Copyright;
pub(crate) use ln0010_no_copyright_file::NoCopyrightFile;
pub(crate) use ln0112_old_fsf_address_in_copyright_file::OldFsfAddressInCopyrightFile;
pub(crate) use ln0113_copyright_refers_to_old_directory::CopyrightRefersToOldDirectory;
pub(crate) use ln0114_copyright_refers_to_deprecated_bsd_license_file::CopyrightRefersToDeprecatedBsdLicenseFile;
pub(crate) use ln0115_copyright_refers_to_compressed_license::CopyrightRefersToCompressedLicense;
pub(crate) use ln0116_copyright_refers_to_incorrect_directory::CopyrightRefersToIncorrectDirectory;
pub(crate) use ln0117_copyright_refers_to_versionless_license_file::CopyrightRefersToVersionlessLicenseFile;
pub(crate) use ln0118_copyright_refers_to_nonexistent_license_file::CopyrightRefersToNonexistentLicenseFile;
pub(crate) use ln0119_copyright_file_compressed::CopyrightFileCompressed;
pub(crate) use ln0120_copyright_file_is_symlink::CopyrightFileIsSymlink;
pub(crate) use ln0121_space_in_std_shortname_in_dep5_copyright::SpaceInStdShortnameInDep5Copyright;
