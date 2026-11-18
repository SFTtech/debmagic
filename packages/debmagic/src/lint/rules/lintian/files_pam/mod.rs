//! Rules for lintian's `files/pam` Check.
//!
//! Installed `etc/pam.conf`, which only `libpam-runtime` may ship.

mod ln0822_config_file_reserved;

pub(crate) use ln0822_config_file_reserved::ConfigFileReserved;
