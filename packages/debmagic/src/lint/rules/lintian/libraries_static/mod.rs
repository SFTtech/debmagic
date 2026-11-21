//! Rules for lintian's `libraries/static/name` Check.

mod ln0768_odd_static_library_name;
mod ln1129_no_code_sections;

pub(crate) use ln0768_odd_static_library_name::OddStaticLibraryName;
pub(crate) use ln1129_no_code_sections::NoCodeSections;
