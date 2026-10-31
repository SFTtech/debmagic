//! `Data::Validate::Domain::is_domain` with default options.
//!
//! A host needs at least two labels, each a hostname label, and a final label
//! that `Net::Domain::TLD` recognises. Single-label names and unknown TLDs
//! (`localhost`, `localdomain`, `domain`) are not domains.

use super::tlds::TLDS;

pub(super) fn is_domain(value: &str) -> bool {
    let Some((hostname, labels)) = domain_labels(value) else {
        return false;
    };
    let _ = hostname;
    if labels.len() < 2 {
        return false;
    }
    let tld = labels.last().map(|label| label.to_ascii_lowercase());
    tld.is_some_and(|tld| TLDS.binary_search(&tld.as_str()).is_ok())
}

fn domain_labels(value: &str) -> Option<(String, Vec<String>)> {
    let trailing_dot = value.ends_with('.');
    let bare = value.strip_suffix('.').unwrap_or(value);
    let encoded_length = bare.len() + if trailing_dot { 1 } else { 2 };
    if encoded_length > 255 {
        return None;
    }
    let mut labels = Vec::new();
    for label in bare.split('.') {
        labels.push(domain_label(label)?.to_string());
    }
    if labels.is_empty() {
        return None;
    }
    let mut hostname = labels.join(".");
    if trailing_dot {
        hostname.push('.');
    }
    Some((hostname, labels))
}

fn domain_label(value: &str) -> Option<&str> {
    if value.is_empty() || value.contains(['\n', '.']) {
        return None;
    }
    let bytes = value.as_bytes();
    let ok = match bytes.len() {
        1 => bytes[0].is_ascii_alphanumeric(),
        2..=63 => {
            bytes[0].is_ascii_alphanumeric()
                && bytes[bytes.len() - 1].is_ascii_alphanumeric()
                && bytes[1..bytes.len() - 1]
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
        }
        _ => false,
    };
    ok.then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_data_validate_domain_defaults() {
        assert!(is_domain("example.com"));
        assert!(is_domain("lists.debian.org"));
        assert!(is_domain("somewhere.com"));
        assert!(!is_domain("localhost"));
        assert!(!is_domain("localhost.localdomain"));
        assert!(!is_domain("localhost.domain"));
        assert!(!is_domain("unknown.domain"));
        assert!(!is_domain("unknown"));
        assert!(!is_domain("heard.of"));
        assert!(!is_domain("a.b"));
        assert!(!is_domain("example.local"));
    }
}
