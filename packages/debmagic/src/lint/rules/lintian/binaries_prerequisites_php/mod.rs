//! Rules for lintian's `binaries/prerequisites/php` Check.

mod ln1010_missing_dependency_on_phpapi;

pub(crate) use ln1010_missing_dependency_on_phpapi::MissingDependencyOnPhpapi;
