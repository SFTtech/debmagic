use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};
use crate::lint::rules::lintian::fields_version::is_native;

pub struct PublicUpstreamKeyNotMinimal;

declare_rule! {
    /// A public upstream signing key carries signatures by other keys, so it
    /// is not a minimal export.
    ///
    /// The key files are lintian's signing key names under `debian/` (see
    /// `public-upstream-key-unusable`). For each public key packet of a
    /// non-native package, the issuer key ids of the signature packets after it,
    /// up to the next public key packet, are compared with the key's own key
    /// id. Each signature by another key is one extra signature. Extra is
    /// `has <n> extra signature(s) for keyid <KEYID>`, with the key id as 16
    /// upper-case hex digits; the Location is the key file. Signatures that name
    /// no issuer are not counted, and a key without a version 4 key id is
    /// skipped. Info and not selected by default, as in lintian.
    ///
    /// The signature packets are read by `lint::openpgp`, not by gpg.
    PublicUpstreamKeyNotMinimal,
    code = "LN1766",
    tag = "public-upstream-key-not-minimal",
    default_selected = false,
    experimental = false,
}

impl SourceRule for PublicUpstreamKeyNotMinimal {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if is_native(ctx) {
            return;
        }
        let Some(files) = ctx.signing_key_files() else {
            return;
        };
        for file in files.iter() {
            // A key id can repeat within one file (the same key packed twice);
            // the Output keeps one copy of an identical hint, as UDD does.
            let mut reported = std::collections::BTreeSet::new();
            for key in file.key.public_keys() {
                let Some(key_id) = key.key_id() else {
                    continue;
                };
                let extra = key
                    .signature_key_ids()
                    .iter()
                    .flatten()
                    .filter(|issuer| **issuer != key_id)
                    .count();
                if extra == 0 {
                    continue;
                }
                let keyid = format!("{:016X}", u64::from_be_bytes(key_id));
                let message = format!("has {extra} extra signature(s) for keyid {keyid}");
                if !reported.insert(message.clone()) {
                    continue;
                }
                ctx.diagnostic(
                    Diagnostic::warn(message)
                        .with_severity(Severity::Info)
                        .with_location(Location {
                            path: file.path.clone(),
                            line: None,
                            column: None,
                        }),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::openpgp::OpenPgpFile;
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "Source: example\n\
         Maintainer: Example <ex@example.com>\n\
         Standards-Version: 4.7.2\n\
         \n\
         Package: example\n\
         Architecture: all\n\
         Description: example\n extra\n";

    const KEY: &str = "debian/upstream/signing-key.asc";

    fn entry(version: &str) -> String {
        format!(
            "example ({version}) unstable; urgency=low\n\n  * Change.\n\n -- A Uthor <a@example.com>  Tue, 30 Dec 2008 17:34:02 +0000\n"
        )
    }

    /// A version 4 public key packet (old-style length) with a fixed body.
    fn key_packet() -> Vec<u8> {
        let body = [4u8, 0, 0, 0, 0, 22, 1, 2, 3, 4, 5, 6, 7, 8];
        let mut data = vec![0xc6, u8::try_from(body.len()).unwrap_or(0)];
        data.extend(body);
        data
    }

    /// The key id of `key_packet()`, as the reader computes it.
    fn key_id() -> [u8; 8] {
        OpenPgpFile::parse(&key_packet()).public_keys()[0]
            .key_id()
            .unwrap_or_default()
    }

    /// The key packet, then one version 4 signature packet per issuer.
    fn signed_key(issuers: &[[u8; 8]]) -> TestFile {
        let mut data = key_packet();
        for issuer in issuers {
            let mut body = vec![4, 0x10, 1, 8, 0, 10, 9, 16];
            body.extend(issuer);
            body.extend([0, 0, 0, 0]);
            data.extend([0x88, u8::try_from(body.len()).unwrap_or(0)]);
            data.extend(body);
        }
        TestFile::from_bytes(data)
    }

    fn tree(
        format: &str,
        version: &str,
        extra: &[(&'static str, TestFile)],
    ) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/changelog", TestFile::new(entry(version))),
            ("debian/source/format", TestFile::new(format)),
        ];
        files.extend(extra.iter().cloned());
        files
    }

    fn package(extra: &[(&'static str, TestFile)]) -> SourcePackageCase {
        let text = "Format: 3.0 (quilt)\nSource: example\nBinary: example\nArchitecture: all\nVersion: 1.0-1\n\
             Maintainer: Example <ex@example.com>\nFiles:\n d41d8cd98f00b204e9800998ecf8427e 0 example_1.0-1.tar.xz\n";
        let dsc = Dsc::parse(text).expect("valid dsc");
        let mut files = vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/changelog", TestFile::new(entry("1.0-1"))),
        ];
        files.extend(extra.iter().cloned());
        SourcePackageCase::from(files).dsc(dsc)
    }

    #[test]
    fn source_tree() {
        let other = [9, 9, 9, 9, 9, 9, 9, 9];
        let pass = vec![
            tree("3.0 (quilt)", "1.0-1", &[]),
            // Only the key's own signature.
            tree("3.0 (quilt)", "1.0-1", &[(KEY, signed_key(&[key_id()]))]),
            tree("3.0 (quilt)", "1.0-1", &[(KEY, signed_key(&[]))]),
            // Native packages are not checked here.
            tree("3.0 (native)", "1.0", &[(KEY, signed_key(&[other]))]),
            // A key file that does not parse has no keys to count.
            tree(
                "3.0 (quilt)",
                "1.0-1",
                &[(KEY, TestFile::new("nonsense\n"))],
            ),
        ];
        let fail = vec![
            tree("3.0 (quilt)", "1.0-1", &[(KEY, signed_key(&[other]))]),
            tree(
                "3.0 (quilt)",
                "1.0-1",
                &[(KEY, signed_key(&[other, key_id(), other]))],
            ),
        ];
        SourceTreeTester::new(PublicUpstreamKeyNotMinimal, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let other = [9, 9, 9, 9, 9, 9, 9, 9];
        let pass = vec![package(&[(KEY, signed_key(&[key_id()]))])];
        let fail = vec![package(&[(KEY, signed_key(&[other]))])];
        SourcePackageTester::new(PublicUpstreamKeyNotMinimal, pass, fail).test_and_snapshot();
    }
}
