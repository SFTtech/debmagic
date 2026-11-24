use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::SourceRule;

use super::mailbox::mailboxes;

pub struct DuplicateContact;

declare_rule! {
    /// An address appears more than once in `Uploaders`.
    ///
    /// Every mailbox of the field is counted by its address, including
    /// identical mailboxes and those with an incomplete address, as lintian
    /// does. Reads the field from the Dsc when there is one, otherwise the
    /// source paragraph of `debian/control`. Each repeated address is reported
    /// once, in field order. Extra is `Uploaders <address>`. Binary packages
    /// have no Uploaders, so only Source subjects are checked.
    DuplicateContact,
    code = "LN0807",
    tag = "duplicate-contact",
    default_selected = true,
    experimental = false,
}

impl SourceRule for DuplicateContact {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let addresses = match ctx.dsc() {
            Some(dsc) => repeated_uploaders(dsc.fields()),
            None => ctx
                .debian_control()
                .map(|control| repeated_uploaders(control.source()))
                .unwrap_or_default(),
        };
        for address in addresses {
            ctx.diagnostic(Diagnostic::warn(format!("Uploaders {address}")));
        }
    }
}

fn repeated_uploaders(fields: &ControlParagraph) -> Vec<String> {
    let Some(value) = fields.unfolded_value("Uploaders") else {
        return Vec::new();
    };
    let addresses: Vec<String> = mailboxes(&value)
        .into_iter()
        .map(|mailbox| mailbox.address)
        .collect();
    let mut repeated: Vec<String> = Vec::new();
    for address in &addresses {
        let count = addresses.iter().filter(|other| *other == address).count();
        if count > 1 && !repeated.contains(address) {
            repeated.push(address.clone());
        }
    }
    repeated
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    fn control(uploaders: &str) -> String {
        format!(
            "Source: example\n\
             Maintainer: Example <ex@example.com>\n\
             Uploaders: {uploaders}\n\
             \n\
             Package: example\n\
             Architecture: all\n\
             Description: example\n extra\n"
        )
    }

    fn tree(uploaders: &str) -> Vec<(&'static str, TestFile)> {
        vec![("debian/control", TestFile::new(control(uploaders)))]
    }

    fn package(uploaders: &str) -> SourcePackageCase {
        let text = format!(
            "Source: example\nBinary: example\nArchitecture: all\nVersion: 1.0-1\n\
             Maintainer: Example <ex@example.com>\nUploaders: {uploaders}\nFiles:\n"
        );
        let dsc = Dsc::parse(&text).expect("valid dsc");
        SourcePackageCase::from(Vec::<(&str, TestFile)>::new()).dsc(dsc)
    }

    #[test]
    fn tree_cases() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            tree("A <a@example.com>"),
            tree("A <a@example.com>, B <b@example.com>"),
            tree("A <a@example.com>, A <a@example.org>"),
            tree("nobody"),
        ];
        let fail = vec![
            tree("A <a@example.com>, A <a@example.com>"),
            tree("A <a@example.com>, B <a@example.com>"),
            tree("A <a@example.com>, <a@example.com>"),
            tree("A <a@example.com>, B <b@example.com>, A <a@example.com>"),
        ];
        SourceTreeTester::new(DuplicateContact, pass, fail).test_and_snapshot();
    }

    #[test]
    fn package_cases() {
        let pass = vec![package("A <a@example.com>, B <b@example.com>")];
        let fail = vec![package("A <a@example.com>, A <a@example.com>")];
        SourcePackageTester::new(DuplicateContact, pass, fail).test_and_snapshot();
    }
}
