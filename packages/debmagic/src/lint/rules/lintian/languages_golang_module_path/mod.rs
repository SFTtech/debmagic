//! Rules for lintian's `languages/golang/module-path` Check.

mod ln1104_go_library_package_does_not_include_version;
mod ln1568_missing_xs_go_import_path_for_golang_package;

pub(crate) use ln1104_go_library_package_does_not_include_version::GoLibraryPackageDoesNotIncludeVersion;
pub(crate) use ln1568_missing_xs_go_import_path_for_golang_package::MissingXsGoImportPathForGolangPackage;
