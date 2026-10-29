//! The single mailbox of a `Maintainer` field, as lintian's `fields/maintainer`
//! Check reads it through `Email::Address::XS`.

/// User and host of a mailbox with a non-empty address.
pub(super) struct Mailbox<'a> {
    pub(super) user: &'a str,
    pub(super) host: &'a str,
}

/// A single `Name <user@host>` or bare `user@host` mailbox. Anything else
/// (several addresses, no address, an empty user or host) is not valid.
pub(super) fn parse_mailbox(value: &str) -> Option<Mailbox<'_>> {
    let value = value.trim();
    let address = match (value.rfind('<'), value.rfind('>')) {
        (Some(open), Some(close)) if open < close && close == value.len() - 1 => {
            &value[open + 1..close]
        }
        (None, None) if !value.contains(char::is_whitespace) => value,
        _ => return None,
    };
    let (user, host) = address.rsplit_once('@')?;
    if user.is_empty() || host.is_empty() || address.contains(',') {
        return None;
    }
    Some(Mailbox { user, host })
}
