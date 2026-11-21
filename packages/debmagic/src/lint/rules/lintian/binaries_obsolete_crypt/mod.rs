//! Rules for lintian's `binaries/obsolete/crypt` Check.

mod ln1159_obsolete_des_encryption;
mod ln1160_obsolete_crypt_alias;

pub(crate) use ln1159_obsolete_des_encryption::ObsoleteDesEncryption;
pub(crate) use ln1160_obsolete_crypt_alias::ObsoleteCryptAlias;
