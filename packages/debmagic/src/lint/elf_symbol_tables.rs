//! Vendored lintian tables keyed by the name of an ELF dynamic symbol.

use crate::lint::data;

/// `binaries/hardened-functions`: C functions that have a `__<name>_chk`
/// hardened variant.
pub(crate) fn has_hardened_variant(name: &str) -> bool {
    data::lines(include_str!("data/binaries/hardened-functions")).any(|line| line == name)
}

/// `binaries/obsolete-crypt-functions`: the Tag lintian reports for a use
/// of a `libcrypt` function that should no longer be used.
pub(crate) fn obsolete_crypt_tag(name: &str) -> Option<&'static str> {
    data::lines(include_str!("data/binaries/obsolete-crypt-functions")).find_map(|line| {
        let (function, tag) = line.split_once("||")?;
        (function.trim() == name).then_some(tag.trim())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardened_functions() {
        assert!(has_hardened_variant("sprintf"));
        assert!(has_hardened_variant("strcpy"));
        // Deliberately omitted for false positives.
        assert!(!has_hardened_variant("memcpy"));
        assert!(has_hardened_variant("printf"));
        assert!(!has_hardened_variant("puts"));
    }

    #[test]
    fn obsolete_crypt_functions() {
        assert_eq!(
            obsolete_crypt_tag("setkey"),
            Some("obsolete-des-encryption")
        );
        assert_eq!(
            obsolete_crypt_tag("encrypt_r"),
            Some("obsolete-des-encryption")
        );
        assert_eq!(obsolete_crypt_tag("fcrypt"), Some("obsolete-crypt-alias"));
        assert_eq!(obsolete_crypt_tag("crypt"), None);
    }
}
