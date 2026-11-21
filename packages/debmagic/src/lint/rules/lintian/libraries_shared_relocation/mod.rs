//! Rules for lintian's `libraries/shared/relocation` Check.

mod ln1156_specific_address_in_shared_library;

pub(crate) use ln1156_specific_address_in_shared_library::SpecificAddressInSharedLibrary;
