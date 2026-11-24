use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::ChangesContext;
use crate::lint::rule::{BinaryPackageRule, ChangesRule, SourceRule};

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
const CHANGES_ROLES: &[&str] = &["Maintainer", "Changed-By", "Uploaders"];

impl SourceRule for RootInContact {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let messages = match ctx.dsc() {
            Some(dsc) => messages(dsc.fields(), ROLES),
            None => ctx
                .debian_control()
                .map(|control| messages(control.source(), ROLES))
                .unwrap_or_default(),
        };
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

impl BinaryPackageRule for RootInContact {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages = messages(ctx.installation_control().fields(), ROLES);
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

impl ChangesRule for RootInContact {
    fn run(&self, ctx: &mut ChangesContext<'_>) {
        let messages = messages(ctx.changes().fields(), CHANGES_ROLES);
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

fn messages(fields: &ControlParagraph, roles: &[&str]) -> Vec<String> {
    let mut found = Vec::new();
    for role in roles {
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
    use crate::lint::tester::{
        BinaryPackageCase, BinaryPackageTester, ChangesCase, ChangesTester, SourceTreeTester,
        TestFile,
    };

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
    fn changes_file() {
        let changes =
            |fields: &str| ChangesCase::from(format!("Source: example\nVersion: 1.0\n{fields}"));
        let pass = vec![
            changes("Changed-By: Jane <jane@example.com>\n"),
            // No phrase is not judged.
            changes("Changed-By: root@example.com\n"),
            changes("Uploaders: Jane <jane@example.com>\n"),
        ];
        let fail = vec![
            changes("Changed-By: Admin <root@example.com>\n"),
            changes("Maintainer: root <jane@example.com>\n"),
        ];
        ChangesTester::new(RootInContact, pass, fail).test_and_snapshot();
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
