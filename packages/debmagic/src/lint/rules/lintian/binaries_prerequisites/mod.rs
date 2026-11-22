//! Rules for lintian's `binaries/prerequisites` Check.

mod ln1295_missing_dependency_on_libc;
#[allow(non_snake_case)]
mod ln1296_missing_dependency_on_libstdc__;
mod shared;

pub(crate) use ln1295_missing_dependency_on_libc::MissingDependencyOnLibc;
pub(crate) use ln1296_missing_dependency_on_libstdc__::MissingDependencyOnLibstdc;
