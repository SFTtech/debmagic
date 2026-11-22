//! Rules for lintian's `binaries/prerequisites` Check.

mod ln1295_missing_dependency_on_libc;
mod shared;

pub(crate) use ln1295_missing_dependency_on_libc::MissingDependencyOnLibc;
