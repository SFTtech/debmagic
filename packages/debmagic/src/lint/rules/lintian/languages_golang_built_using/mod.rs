//! Rules for lintian's `languages/golang/built-using` Check.

mod ln1687_missing_static_built_using_field_for_golang_package;

pub(crate) use ln1687_missing_static_built_using_field_for_golang_package::MissingStaticBuiltUsingFieldForGolangPackage;
