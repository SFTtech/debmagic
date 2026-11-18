use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

use super::mailbox::{Mailbox, complete_mailboxes};

pub struct RootInContact;

declare_rule! {
    /// A Maintainer or Uploaders mailbox with a phrase names root.
    ///
    /// A mailbox is reported when its phrase is `root` or its user part is
    /// `root`. A mailbox with no phrase is not judged. Reads those fields from
    /// the Dsc when there is one, otherwise the source paragraph, and from
    /// Installation control on a Binary package. Extra is the field name and
    /// the mailbox. `.changes` has no Subject.
    RootInContact,
    code = "LN0731",
    tag = "root-in-contact",
    default_selected = true,
    experimental = false,
}

const ROLES: &[&str] = &["Maintainer", "Uploaders"];

impl SourceRule for RootInContact {
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

impl BinaryPackageRule for RootInContact {
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
        for mailbox in complete_mailboxes(fields, role) {
            if mailbox.phrase.is_empty() {
                continue;
            }
            if mailbox.phrase == "root" || mailbox.user == "root" {
                found.push(format!("{role} {}", quoted_format(&mailbox)));
            }
        }
    }
    found
}

/// Email::Address::XS quotes a phrase that is not a plain atom sequence.
fn quoted_format(mailbox: &Mailbox) -> String {
    let plain = mailbox
        .phrase
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-._'".contains(c));
    if plain {
        mailbox.format()
    } else {
        format!("\"{}\" <{}>", mailbox.phrase, mailbox.address)
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(fields: &str) -> String {
        format!(
            "Source: example\n{fields}\nPackage: example\nArchitecture: all\nDescription: example\n extra\n"
        )
    }

    fn tree(fields: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/control", TestFile::new(control(fields)))]
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree("Maintainer: A Maintainer <a@example.com>\n"),
            tree("Maintainer: root@debian.org\n"),
            tree("Maintainer: Rooter <rooter@example.com>\n"),
            tree("Maintainer: A <a@example.com>\nUploaders: Roota <roota@example.com>\n"),
        ];
        let fail = vec![
            tree("Maintainer: root <someone@debian.org>\n"),
            tree("Maintainer: \"John Doe\" <root@debian.org>\n"),
            tree("Maintainer: A <a@example.com>\nUploaders: B <root@example.com>\n"),
        ];
        SourceTreeTester::new(RootInContact, pass, fail).test_and_snapshot();
    }

    #[test]
    fn binary_package() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Maintainer", "A <a@example.com>"),
        ];
        let fail = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Maintainer", "root <someone@debian.org>"),
        ];
        BinaryPackageTester::new(RootInContact, pass, fail).test_and_snapshot_with_suffix("binary");
    }
}
