//! Rules for lintian's `substvars/libc` Check.

mod ln1135_package_depends_on_hardcoded_libc;

pub(crate) use ln1135_package_depends_on_hardcoded_libc::PackageDependsOnHardcodedLibc;
