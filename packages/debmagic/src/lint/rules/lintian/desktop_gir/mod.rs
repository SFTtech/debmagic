//! Rules for lintian's `desktop/gnome/gir` Check.
//!
//! Each Rule owns one Tag. GIR XML files are the direct children of
//! `usr/share/gir-1.0` and `usr/lib/<triplet>/gir-1.0`.

mod gir_paths;
mod ln0435_gir_section_not_libdevel;
mod ln0916_typelib_missing_gir_depends;
mod ln0918_gir_package_name_does_not_match;
mod ln0919_typelib_not_in_multiarch_directory;
mod ln0920_typelib_package_name_does_not_match;
mod ln0921_typelib_section_not_introspection;
mod ln0922_gir_in_arch_all_package;
mod ln1348_typelib_in_arch_all_package;

pub(crate) use ln0435_gir_section_not_libdevel::GirSectionNotLibdevel;
pub(crate) use ln0916_typelib_missing_gir_depends::TypelibMissingGirDepends;
pub(crate) use ln0918_gir_package_name_does_not_match::GirPackageNameDoesNotMatch;
pub(crate) use ln0919_typelib_not_in_multiarch_directory::TypelibNotInMultiarchDirectory;
pub(crate) use ln0920_typelib_package_name_does_not_match::TypelibPackageNameDoesNotMatch;
pub(crate) use ln0921_typelib_section_not_introspection::TypelibSectionNotIntrospection;
pub(crate) use ln0922_gir_in_arch_all_package::GirInArchAllPackage;
pub(crate) use ln1348_typelib_in_arch_all_package::TypelibInArchAllPackage;
