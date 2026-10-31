use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

use super::domain::is_domain;

pub struct BogusMailHost;

declare_rule! {
    /// A Maintainer or Uploaders address uses a host that is not a domain.
    ///
    /// Reads those fields from the Dsc when there is one, otherwise the source
    /// paragraph, and from Installation control on a Binary package. An address
    /// needs a user and a host; `Data::Validate::Domain`'s default `is_domain`
    /// decides the host (`localhost.localdomain` is not one). Extra is the
    /// field name and the address. `.changes` has no Subject.
    BogusMailHost,
    code = "LN0106",
    tag = "bogus-mail-host",
    default_selected = true,
    experimental = false,
}

const ROLES: &[&str] = &["Maintainer", "Uploaders"];

impl SourceRule for BogusMailHost {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = match ctx.dsc() {
            Some(dsc) => messages(dsc.fields()),
            None => ctx
                .debian_control()
                .map(|control| messages(control.source()))
                .unwrap_or_default(),
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

impl BinaryPackageRule for BogusMailHost {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages = messages(ctx.installation_control().fields());
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

fn messages(fields: &ControlParagraph) -> Vec<String> {
    let mut found = Vec::new();
    for role in ROLES {
        let Some(value) = fields.unfolded_value(role) else {
            continue;
        };
        let mut seen = Vec::new();
        for mail in mailboxes(&value) {
            if seen.iter().any(|address: &String| address == &mail.address) {
                continue;
            }
            seen.push(mail.address.clone());
            if !is_domain(&mail.host) {
                found.push(format!("{role} {}", mail.address));
            }
        }
    }
    found
}

struct Mail {
    address: String,
    host: String,
}

fn mailboxes(value: &str) -> Vec<Mail> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    for character in value.chars() {
        match character {
            '<' => {
                depth += 1;
                current.push(character);
            }
            '>' => {
                depth -= 1;
                current.push(character);
            }
            ',' if depth == 0 => parts.push(std::mem::take(&mut current)),
            _ => current.push(character),
        }
    }
    parts.push(current);
    parts
        .iter()
        .filter_map(|part| parse_mail(part.trim()))
        .collect()
}

fn parse_mail(part: &str) -> Option<Mail> {
    if part.is_empty() {
        return None;
    }
    let address = if let Some(open) = part.rfind('<') {
        let close = part.rfind('>')?;
        if close <= open {
            return None;
        }
        part[open + 1..close].trim()
    } else if part.contains(char::is_whitespace) {
        return None;
    } else {
        part
    };
    let (user, host) = address.rsplit_once('@')?;
    if user.is_empty() || host.is_empty() {
        return None;
    }
    Some(Mail {
        address: address.to_string(),
        host: host.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{
        BinaryPackageCase, BinaryPackageTester, SourcePackageTester, SourceTreeTester, TestFile,
    };

    use super::*;

    fn control(maintainer: &str, uploaders: &str) -> Vec<(&'static str, TestFile)> {
        let uploaders = if uploaders.is_empty() {
            String::new()
        } else {
            format!("Uploaders: {uploaders}\n")
        };
        vec![(
            "debian/control",
            TestFile::new(format!(
                "Source: example\nMaintainer: {maintainer}\n{uploaders}\
                 Standards-Version: 4.7.2\n\n\
                 Package: example\nArchitecture: all\nDescription: example\n extra\n"
            )),
        )]
    }

    #[test]
    fn test() {
        let pass = vec![
            control("Jane <jane@example.com>", ""),
            control(
                "Team <team@lists.debian.org>",
                "John A. J. Doe <i.am.ok@somewhere.com>, Valid email <0@somewhere.com>",
            ),
        ];
        let fail = vec![
            control("Mrs. Localhost <user@localhost.localdomain>", ""),
            control(
                "Team <team@lists.debian.org>",
                "Mrs. Localhost <user@localhost.domain>, Ok <ok@example.com>",
            ),
            control(
                "Team <team@debian.org>",
                "One <he@unknown.domain>, Two <jeroen@localhost.localdomain>",
            ),
        ];
        SourceTreeTester::new(BogusMailHost, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Maintainer", "Jane <jane@example.com>"),
        ];
        let fail = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Maintainer", "Mrs. Localhost <user@localhost.localdomain>"),
        ];
        BinaryPackageTester::new(BogusMailHost, pass, fail).test_and_snapshot_with_suffix("binary");
    }

    #[test]
    fn test_source_package() {
        let pass = vec![crate::lint::tester::SourcePackageCase::from(control(
            "Jane <jane@example.com>",
            "",
        ))];
        let fail = vec![crate::lint::tester::SourcePackageCase::from(control(
            "Mrs. Localhost <user@localhost.localdomain>",
            "",
        ))];
        SourcePackageTester::new(BogusMailHost, pass, fail).test_and_snapshot();
    }
}
