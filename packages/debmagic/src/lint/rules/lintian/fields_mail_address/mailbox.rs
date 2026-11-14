//! Mailboxes of `Maintainer`, `Uploaders`, and `Changed-By` values, as
//! lintian's `fields/mail-address` Check reads them through `Email::Address::XS`.
//!
//! A part is `Phrase <user@host>` or a bare `user@host`. Commas inside `<>` or
//! double quotes do not split. A part with no address is dropped. The user and
//! host may be empty, as Email::Address::XS returns them.

use crate::lint::debian_control::ControlParagraph;

/// One parsed mailbox of a mail-address field.
pub(super) struct Mailbox {
    pub(super) phrase: String,
    pub(super) address: String,
    pub(super) user: String,
    pub(super) host: String,
}

impl Mailbox {
    /// Lintian's `format`: the phrase with the address, or the bare address.
    pub(super) fn format(&self) -> String {
        if self.phrase.is_empty() {
            self.address.clone()
        } else {
            format!("{} <{}>", self.phrase, self.address)
        }
    }
}

pub(super) fn mailboxes(value: &str) -> Vec<Mailbox> {
    split_top_level(value)
        .iter()
        .filter_map(|part| parse_part(part.trim()))
        .collect()
}

/// Each distinct mailbox of `role` on the paragraph that has a user and a
/// host, in field order. Duplicates are judged by `format`, as lintian's
/// `uniq_by` does, before the user and host test.
pub(super) fn complete_mailboxes(fields: &ControlParagraph, role: &str) -> Vec<Mailbox> {
    let Some(value) = fields.unfolded_value(role) else {
        return Vec::new();
    };
    let mut seen: Vec<String> = Vec::new();
    let mut found = Vec::new();
    for mailbox in mailboxes(&value) {
        let format = mailbox.format();
        if seen.contains(&format) {
            continue;
        }
        seen.push(format);
        if mailbox.user.is_empty() || mailbox.host.is_empty() {
            continue;
        }
        found.push(mailbox);
    }
    found
}

fn split_top_level(value: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut angle = false;
    let mut quoted = false;
    for character in value.chars() {
        match character {
            '"' => {
                quoted = !quoted;
                current.push(character);
            }
            '<' if !quoted => {
                angle = true;
                current.push(character);
            }
            '>' if !quoted => {
                angle = false;
                current.push(character);
            }
            ',' if !angle && !quoted => parts.push(std::mem::take(&mut current)),
            _ => current.push(character),
        }
    }
    parts.push(current);
    parts
}

fn parse_part(part: &str) -> Option<Mailbox> {
    if part.is_empty() {
        return None;
    }
    let (phrase, address) = match (part.rfind('<'), part.rfind('>')) {
        (Some(open), Some(close)) if open < close => (
            part[..open].trim().trim_matches('"').trim().to_string(),
            part[open + 1..close].trim().to_string(),
        ),
        (None, None) if !part.contains(char::is_whitespace) => (String::new(), part.to_string()),
        _ => return None,
    };
    let (user, host) = address.rsplit_once('@')?;
    Some(Mailbox {
        phrase,
        user: user.to_string(),
        host: host.to_string(),
        address,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_top_level_commas_only() {
        let parsed = mailboxes("\"Doe, Jane\" <jane@example.com>, bob@example.org");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].phrase, "\"Doe, Jane\"".trim_matches('"'));
        assert_eq!(parsed[0].address, "jane@example.com");
        assert_eq!(parsed[1].address, "bob@example.org");
        assert_eq!(parsed[1].format(), "bob@example.org");
    }

    #[test]
    fn parts_without_an_address_are_dropped() {
        assert!(mailboxes("Debian QA Group").is_empty());
        assert!(mailboxes("no-at-sign <nobody>").is_empty());
    }
}
