use crate::declare_rule;
use crate::lint::context::{BinaryPackageContext, SourceContext};
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::group_context::ChangesContext;
use crate::lint::rule::{BinaryPackageRule, ChangesRule, SourceRule};

use super::mailbox::{Mailbox, complete_mailboxes};

pub struct FaultyDebianQaGroupPhrase;

declare_rule! {
    /// A Maintainer or Uploaders mailbox has the QA group address under another name.
    ///
    /// A mailbox with address `packages@qa.debian.org` whose phrase is not
    /// exactly `Debian QA Group` is reported. A mailbox with no phrase is not
    /// judged. Reads the Dsc when there is one, otherwise the source paragraph,
    /// and Installation control on a Binary package. Extra is the field name,
    /// the phrase, and `-> Debian QA Group`.
    FaultyDebianQaGroupPhrase,
    code = "LN0372",
    tag = "faulty-debian-qa-group-phrase",
    default_selected = true,
    experimental = false,
}

const ROLES: &[&str] = &["Maintainer", "Uploaders"];
const CHANGES_ROLES: &[&str] = &["Maintainer", "Changed-By", "Uploaders"];
const QA_GROUP_ADDRESS: &str = "packages@qa.debian.org";
const QA_GROUP_PHRASE: &str = "Debian QA Group";

impl SourceRule for FaultyDebianQaGroupPhrase {
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

impl BinaryPackageRule for FaultyDebianQaGroupPhrase {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let messages = messages(ctx.installation_control().fields(), ROLES);
        for message in messages {
            ctx.diagnostic(Diagnostic::error(message));
        }
    }
}

impl ChangesRule for FaultyDebianQaGroupPhrase {
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
            if faulty(&mailbox) {
                found.push(format!("{role} {} -> {QA_GROUP_PHRASE}", mailbox.phrase));
            }
        }
    }
    found
}

fn faulty(mailbox: &Mailbox) -> bool {
    !mailbox.phrase.is_empty()
        && mailbox.address == QA_GROUP_ADDRESS
        && mailbox.phrase != QA_GROUP_PHRASE
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
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree("Maintainer: Debian QA Group <packages@qa.debian.org>\n"),
            tree("Maintainer: packages@qa.debian.org\n"),
            tree(
                "Maintainer: Someone <a@example.com>\nUploaders: Debian QA <debian-qa@example.com>\n",
            ),
        ];
        let fail = vec![
            tree("Maintainer: Debian QA <packages@qa.debian.org>\n"),
            tree(
                "Maintainer: A <a@example.com>\nUploaders: \"QA group\" <packages@qa.debian.org>\n",
            ),
        ];
        SourceTreeTester::new(FaultyDebianQaGroupPhrase, pass, fail).test_and_snapshot();
    }

    #[test]
    fn changes_file() {
        let changes =
            |fields: &str| ChangesCase::from(format!("Source: example\nVersion: 1.0\n{fields}"));
        let pass = vec![
            changes("Changed-By: Debian QA Group <packages@qa.debian.org>\n"),
            changes("Changed-By: Jane <jane@example.com>\n"),
            // No phrase is not judged.
            changes("Maintainer: packages@qa.debian.org\n"),
        ];
        let fail = vec![
            changes("Changed-By: Debian QA <packages@qa.debian.org>\n"),
            changes(
                "Maintainer: Jane <jane@example.com>\nUploaders: QA Team <packages@qa.debian.org>\n",
            ),
        ];
        ChangesTester::new(FaultyDebianQaGroupPhrase, pass, fail).test_and_snapshot();
    }

    #[test]
    fn binary_package() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Maintainer", "Debian QA Group <packages@qa.debian.org>"),
        ];
        let fail = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Maintainer", "Debian QA <packages@qa.debian.org>"),
        ];
        BinaryPackageTester::new(FaultyDebianQaGroupPhrase, pass, fail)
            .test_and_snapshot_with_suffix("binary");
    }
}
