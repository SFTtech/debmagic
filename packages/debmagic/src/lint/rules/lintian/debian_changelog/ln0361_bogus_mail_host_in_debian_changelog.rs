use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::super::domain::is_domain_ignoring_tld;
use super::installed::installed_changelog;

pub struct BogusMailHostInDebianChangelog;

declare_rule! {
    /// An installed changelog trailer uses an email host that is not a domain.
    ///
    /// Reads the installed changelog the way `debian-changelog-has-wrong-day-of-week`
    /// does, and checks every entry that has a trailer. The host needs two
    /// labels; the TLD list is not consulted (`domain_disable_tld_validation`).
    /// A trailer `Email::Address::XS` would reject is reported as the whole
    /// maintainer. Extra is that address or maintainer, then
    /// `(for version <version>)`. The heading line is the Location. Udebs are
    /// not checked.
    BogusMailHostInDebianChangelog,
    code = "LN0361",
    tag = "bogus-mail-host-in-debian-changelog",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for BogusMailHostInDebianChangelog {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let Some(installed) = installed_changelog(ctx) else {
            return;
        };
        let path = installed.path.clone();
        for entry in installed.parsed.entries() {
            let maintainer = entry.maintainer();
            if maintainer.is_empty() {
                continue;
            }
            let Some(shown) = reported_address(maintainer) else {
                continue;
            };
            let line = entry.line();
            ctx.diagnostic(
                Diagnostic::error(format!("{shown} (for version {})", entry.version()))
                    .with_location(Location {
                        path: path.clone(),
                        line: Some(line),
                        column: None,
                    }),
            );
        }
    }
}

/// Address text for a bad trailer, or `None` when the host is a domain.
///
/// An unparsable trailer reports the whole maintainer. A parsed `user@host`
/// whose host has fewer than two labels reports `user@host`.
fn reported_address(maintainer: &str) -> Option<String> {
    let Some(address) = angle_address(maintainer) else {
        return Some(maintainer.to_string());
    };
    let (user, host) = address.rsplit_once('@')?;
    if user.is_empty() || host.is_empty() || host.ends_with('.') {
        return Some(maintainer.to_string());
    }
    if is_domain_ignoring_tld(host) {
        None
    } else {
        Some(address.to_string())
    }
}

/// The single `<user@host>` at the end of a changelog maintainer.
///
/// `None` when `Email::Address::XS` would mark the trailer invalid: no
/// brackets, trailing junk, whitespace in the address, or a host character
/// the module rejects.
fn angle_address(maintainer: &str) -> Option<&str> {
    let trimmed = maintainer.trim();
    if !trimmed.ends_with('>') {
        return None;
    }
    let open = trimmed.rfind('<')?;
    let address = &trimmed[open + 1..trimmed.len() - 1];
    let (user, host) = address.rsplit_once('@')?;
    if user.is_empty()
        || host.is_empty()
        || user.contains(char::is_whitespace)
        || host.contains(char::is_whitespace)
        || host.ends_with('.')
    {
        return None;
    }
    let host_ok = host.bytes().all(|byte| {
        !matches!(
            byte,
            b'(' | b')' | b'<' | b'>' | b'[' | b']' | b':' | b';' | b'@' | b'\\' | b',' | b'"'
        ) && byte > 32
            && byte != 127
    });
    host_ok.then_some(address)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn changelog(entries: &[(&str, &str, &str)]) -> String {
        entries
            .iter()
            .map(|(version, distribution, mail)| {
                format!(
                    "example ({version}) {distribution}; urgency=low\n\n  * Change.\n\n -- {mail}  Tue, 30 Dec 2008 17:34:02 +0000\n"
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn gzip(text: &str) -> TestFile {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(text.as_bytes()).expect("gzip");
        TestFile::from_bytes(encoder.finish().expect("gzip"))
    }

    fn packaged(name: &str, file: TestFile) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(name.to_string(), file)])
    }

    #[test]
    fn test() {
        let fine = changelog(&[("1.0-1", "unstable", "A Uthor <a@example.com>")]);
        let localdomain =
            changelog(&[("1.0-1", "unstable", "A Uthor <user@localhost.localdomain>")]);
        let pass = vec![
            BinaryPackageCase::from(Vec::<(String, TestFile)>::new()),
            packaged("usr/share/doc/example/changelog.Debian.gz", gzip(&fine)),
            packaged(
                "usr/share/doc/example/changelog",
                TestFile::new(&localdomain),
            ),
            packaged(
                "usr/share/doc/example/changelog",
                TestFile::new(changelog(&[(
                    "1.0-1",
                    "unstable",
                    "A Uthor <user@example.invalid>",
                )])),
            ),
            packaged(
                "usr/share/doc/example/changelog",
                TestFile::new(changelog(&[(
                    "1.0-1",
                    "unstable",
                    "A Uthor <user@heard.of>",
                )])),
            ),
            packaged(
                "usr/share/doc/example/changelog.gz",
                gzip(&changelog(&[(
                    "1.0-1",
                    "unstable",
                    "A Uthor <jeroen@mordor>",
                )])),
            )
            .udeb(),
            packaged(
                "usr/share/doc/example/changelog",
                TestFile::new("not a changelog\n"),
            ),
        ];
        let fail = vec![
            packaged(
                "usr/share/doc/example/changelog.gz",
                gzip(&changelog(&[
                    ("2.0-1", "unstable", "Marc 'HE' Brockschmidt <he@unknown>"),
                    ("1.0-1", "unstable", "Jeroen van Wolffelaar <jeroen@mordor>"),
                ])),
            ),
            packaged(
                "usr/share/doc/example/changelog.Debian",
                TestFile::new(changelog(&[(
                    "1.0-1",
                    "unstable",
                    "Unknown <unknown@unknown>",
                )])),
            ),
            packaged(
                "usr/share/doc/example/changelog",
                TestFile::new(changelog(&[(
                    "1.0-1",
                    "unstable",
                    "A Uthor <not-an-email>",
                )])),
            ),
            packaged(
                "usr/share/doc/example/changelog",
                TestFile::new(changelog(&[(
                    "1.0-1",
                    "unstable",
                    "A Uthor <user@localhost>",
                )])),
            ),
        ];
        BinaryPackageTester::new(BogusMailHostInDebianChangelog, pass, fail).test_and_snapshot();
    }
}
