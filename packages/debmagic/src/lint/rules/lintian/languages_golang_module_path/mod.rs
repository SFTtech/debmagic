//! Rules for lintian's `languages/golang/module-path` Check.

mod ln1104_go_library_package_does_not_include_version;

pub(crate) use ln1104_go_library_package_does_not_include_version::GoLibraryPackageDoesNotIncludeVersion;
