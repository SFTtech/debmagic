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
mod ln0079_star_file;
mod ln0080_package_contains_file_in_etc_skel;
mod ln0081_package_contains_file_in_usr_share_hal;
mod ln0082_package_contains_icon_cache_in_generic_dir;
mod ln0083_package_contains_python_coverage_file;
mod ln0084_package_contains_python_doctree_file;
mod ln0085_package_contains_python_header_in_incorrect_directory;
mod ln0086_package_contains_python_tests_in_global_namespace;
mod ln0087_package_contains_eslint_config_file;
mod ln0088_package_contains_npm_ignore_file;
mod names;

pub(crate) use ln0065_file_name_ends_in_whitespace::FileNameEndsInWhitespace;
pub(crate) use ln0067_file_name_contains_wildcard_character::FileNameContainsWildcardCharacter;
pub(crate) use ln0072_package_contains_python_dot_directory::PackageContainsPythonDotDirectory;
pub(crate) use ln0073_package_contains_python_hypothesis_example::PackageContainsPythonHypothesisExample;
pub(crate) use ln0074_package_contains_sass_cache_directory::PackageContainsSassCacheDirectory;
pub(crate) use ln0076_hyphen_file::HyphenFile;
pub(crate) use ln0077_file_name_in_path_is_not_ascii::FileNameInPathIsNotAscii;
pub(crate) use ln0078_package_contains_compiled_glib_schema::PackageContainsCompiledGlibSchema;
pub(crate) use ln0079_star_file::StarFile;
pub(crate) use ln0080_package_contains_file_in_etc_skel::PackageContainsFileInEtcSkel;
pub(crate) use ln0081_package_contains_file_in_usr_share_hal::PackageContainsFileInUsrShareHal;
pub(crate) use ln0082_package_contains_icon_cache_in_generic_dir::PackageContainsIconCacheInGenericDir;
pub(crate) use ln0083_package_contains_python_coverage_file::PackageContainsPythonCoverageFile;
pub(crate) use ln0084_package_contains_python_doctree_file::PackageContainsPythonDoctreeFile;
pub(crate) use ln0085_package_contains_python_header_in_incorrect_directory::PackageContainsPythonHeaderInIncorrectDirectory;
pub(crate) use ln0086_package_contains_python_tests_in_global_namespace::PackageContainsPythonTestsInGlobalNamespace;
pub(crate) use ln0087_package_contains_eslint_config_file::PackageContainsEslintConfigFile;
pub(crate) use ln0088_package_contains_npm_ignore_file::PackageContainsNpmIgnoreFile;
