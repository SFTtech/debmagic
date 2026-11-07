use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

use super::mailbox::complete_mailboxes;

pub struct MailAddressLoopsOrBounces;

declare_rule! {
    /// A Maintainer or Uploaders address is a list that loops or bounces.
    ///
    /// Reads those fields from the Dsc when there is one, otherwise the source
    /// paragraph, and from Installation control on a Binary package. Only
    /// `ubuntu-devel-discuss@lists.ubuntu.com` is known to bounce. The address
    /// needs a non-empty user and host. Each distinct mailbox is reported once.
    /// Extra is the field name and the bare address. `.changes` has no Subject.
    MailAddressLoopsOrBounces,
    code = "LN0370",
    tag = "mail-address-loops-or-bounces",
    default_selected = true,
    experimental = false,
}

const ROLES: &[&str] = &["Maintainer", "Uploaders"];

const KNOWN_BOUNCE_ADDRESSES: &[&str] = &["ubuntu-devel-discuss@lists.ubuntu.com"];

impl SourceRule for MailAddressLoopsOrBounces {
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

impl BinaryPackageRule for MailAddressLoopsOrBounces {
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
            if KNOWN_BOUNCE_ADDRESSES.contains(&mailbox.address.as_str()) {
                found.push(format!("{role} {}", mailbox.address));
            }
        }
    }
    found
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
            tree("Maintainer: Ubuntu <ubuntu-devel-discuss@lists.ubuntu.com.example>\n"),
            tree(
                "Maintainer: A <a@example.com>\nUploaders: B <ubuntu-devel-discuss@example.com>\n",
            ),
        ];
        let fail = vec![
            tree("Maintainer: Ubuntu <ubuntu-devel-discuss@lists.ubuntu.com>\n"),
            tree(
                "Maintainer: A <a@example.com>\nUploaders: B <b@example.com>, Bounce <ubuntu-devel-discuss@lists.ubuntu.com>\n",
            ),
        ];
        SourceTreeTester::new(MailAddressLoopsOrBounces, pass, fail).test_and_snapshot();
    }

    #[test]
    fn binary_package() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Maintainer", "A <a@example.com>"),
        ];
        let fail = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).field(
                "Maintainer",
                "Ubuntu <ubuntu-devel-discuss@lists.ubuntu.com>",
            ),
        ];
        BinaryPackageTester::new(MailAddressLoopsOrBounces, pass, fail)
            .test_and_snapshot_with_suffix("binary");
    }
}
