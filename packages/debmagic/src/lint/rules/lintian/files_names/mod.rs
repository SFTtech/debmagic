//! Rules for installed-file names, matching lintian's `files/names` Check.
//!
//! Each Rule owns one Tag. `names` is the path predicate; `flag` emits the
//! empty extra. Non-UTF-8 paths are left out (`Path::to_str`).

mod flag;
mod ln0065_file_name_ends_in_whitespace;
mod ln0067_file_name_contains_wildcard_character;
mod ln0072_package_contains_python_dot_directory;
mod ln0073_package_contains_python_hypothesis_example;
mod ln0074_package_contains_sass_cache_directory;
mod ln0076_hyphen_file;
mod ln0077_file_name_in_path_is_not_ascii;
mod ln0078_package_contains_compiled_glib_schema;
mod names;

pub(crate) use ln0065_file_name_ends_in_whitespace::FileNameEndsInWhitespace;
pub(crate) use ln0067_file_name_contains_wildcard_character::FileNameContainsWildcardCharacter;
pub(crate) use ln0072_package_contains_python_dot_directory::PackageContainsPythonDotDirectory;
pub(crate) use ln0073_package_contains_python_hypothesis_example::PackageContainsPythonHypothesisExample;
pub(crate) use ln0074_package_contains_sass_cache_directory::PackageContainsSassCacheDirectory;
pub(crate) use ln0076_hyphen_file::HyphenFile;
pub(crate) use ln0077_file_name_in_path_is_not_ascii::FileNameInPathIsNotAscii;
pub(crate) use ln0078_package_contains_compiled_glib_schema::PackageContainsCompiledGlibSchema;
