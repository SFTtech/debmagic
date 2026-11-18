//! Rules for lintian's `files/obsolete-paths` Check.
//!
//! Installed paths under a directory that lintian lists as obsolete.

mod ln0824_package_installs_into_obsolete_dir;

pub(crate) use ln0824_package_installs_into_obsolete_dir::PackageInstallsIntoObsoleteDir;
