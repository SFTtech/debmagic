//! Rules for lintian's `libraries/static/name` Check.

mod ln0768_odd_static_library_name;
mod ln1416_no_code_sections;
mod ln1574_static_link_time_optimization;
mod ln1751_static_library_has_unneeded_sections;
mod ln1752_unstripped_static_library;

pub(crate) use ln0768_odd_static_library_name::OddStaticLibraryName;
pub(crate) use ln1416_no_code_sections::NoCodeSections;
pub(crate) use ln1574_static_link_time_optimization::StaticLinkTimeOptimization;
pub(crate) use ln1751_static_library_has_unneeded_sections::StaticLibraryHasUnneededSections;
pub(crate) use ln1752_unstripped_static_library::UnstrippedStaticLibrary;
