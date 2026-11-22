//! Rules for lintian's `build-systems/automake` Check.

mod ln1511_deprecated_configure_filename;

pub(crate) use ln1511_deprecated_configure_filename::DeprecatedConfigureFilename;
