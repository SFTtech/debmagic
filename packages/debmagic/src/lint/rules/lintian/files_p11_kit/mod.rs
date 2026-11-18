//! Rules for lintian's `files/p11-kit` Check.
//!
//! Installed files under `usr/share/p11-kit/modules/`.

mod ln0823_incorrect_naming_of_pkcs11_module;

pub(crate) use ln0823_incorrect_naming_of_pkcs11_module::IncorrectNamingOfPkcs11Module;
