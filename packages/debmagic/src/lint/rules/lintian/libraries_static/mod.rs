//! Rules for lintian's `libraries/static/name` Check.

mod ln0768_odd_static_library_name;
mod ln1416_no_code_sections;
mod ln1574_static_link_time_optimization;

pub(crate) use ln0768_odd_static_library_name::OddStaticLibraryName;
pub(crate) use ln1416_no_code_sections::NoCodeSections;
pub(crate) use ln1574_static_link_time_optimization::StaticLinkTimeOptimization;
