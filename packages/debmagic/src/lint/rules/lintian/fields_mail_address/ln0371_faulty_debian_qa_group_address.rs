use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{BinaryPackageRule, SourceRule};

use super::mailbox::{Mailbox, complete_mailboxes};

pub struct FaultyDebianQaGroupAddress;

declare_rule! {
    /// A Maintainer or Uploaders mailbox names the QA group without its address.
    ///
    /// A mailbox with a phrase is faulty when the phrase contains `Debian QA`
    /// (any case, a word boundary either side, whitespace between) and the
    /// address is not `packages@qa.debian.org`. The address
    /// `debian-qa@lists.debian.org` is always faulty. A mailbox with no phrase
    /// is not judged. Reads the Dsc when there is one, otherwise the source
    /// paragraph, and Installation control on a Binary package. Extra is the
    /// field name, the address, and `-> packages@qa.debian.org`.
    FaultyDebianQaGroupAddress,
    code = "LN0371",
    tag = "faulty-debian-qa-group-address",
    default_selected = true,
    experimental = false,
}

const ROLES: &[&str] = &["Maintainer", "Uploaders"];
const QA_GROUP_ADDRESS: &str = "packages@qa.debian.org";
const QA_LIST_ADDRESS: &str = "debian-qa@lists.debian.org";

static DEBIAN_QA_PHRASE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bdebian\s+qa\b").expect("valid regex"));

impl SourceRule for FaultyDebianQaGroupAddress {
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

impl BinaryPackageRule for FaultyDebianQaGroupAddress {
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
            if faulty(&mailbox) {
                found.push(format!("{role} {} -> {QA_GROUP_ADDRESS}", mailbox.address));
            }
        }
    }
    found
}

fn faulty(mailbox: &Mailbox) -> bool {
    if mailbox.phrase.is_empty() {
        return false;
    }
    (DEBIAN_QA_PHRASE.is_match(&mailbox.phrase) && mailbox.address != QA_GROUP_ADDRESS)
        || mailbox.address == QA_LIST_ADDRESS
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
            tree("Maintainer: Debian QA Group <packages@qa.debian.org>\n"),
            tree("Maintainer: debian-qa@lists.debian.org\n"),
            tree(
                "Maintainer: Someone <a@example.com>\nUploaders: Debian Dev <debian-dev@example.com>\n",
            ),
            tree("Maintainer: Debian QA Team <packages@qa.debian.org>\n"),
            tree("Maintainer: Lintian QA <lintian-qa@example.com>\n"),
        ];
        let fail = vec![
            tree("Maintainer: Debian QA Group <debian-qa@lists.debian.org>\n"),
            tree("Maintainer: Debian QA <a@example.com>\n"),
            tree("Maintainer: A <a@example.com>\nUploaders: Debian  qa team <qa@example.com>\n"),
        ];
        SourceTreeTester::new(FaultyDebianQaGroupAddress, pass, fail).test_and_snapshot();
    }

    #[test]
    fn binary_package() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Maintainer", "Debian QA Group <packages@qa.debian.org>"),
        ];
        let fail = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Maintainer", "Debian QA Group <debian-qa@lists.debian.org>"),
        ];
        BinaryPackageTester::new(FaultyDebianQaGroupAddress, pass, fail)
            .test_and_snapshot_with_suffix("binary");
    }
}
