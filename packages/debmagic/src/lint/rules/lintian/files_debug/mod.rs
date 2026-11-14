//! Rules for lintian's `files/debug-packages` Check.

mod ln0426_non_debug_file_in_debug_package;

pub(crate) use ln0426_non_debug_file_in_debug_package::NonDebugFileInDebugPackage;
