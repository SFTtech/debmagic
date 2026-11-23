use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};
use crate::lint::rules::lintian::fields_version::is_native;

pub struct PublicUpstreamKeyUnusable;

declare_rule! {
    /// A public upstream signing key of a non-native package does not read as
    /// OpenPGP data, or holds no public key.
    ///
    /// The key files are lintian's signing key names under `debian/`:
    /// `upstream-signing-key.pgp`, `upstream/signing-key.pgp` and
    /// `upstream/signing-key.asc`. A name that does not resolve to a file
    /// (symlinks are followed to their target) is not read. Extra is
    /// `cannot be processed` when the file is empty, is not OpenPGP data, has
    /// an armor checksum that does not match, or has an invalid packet after
    /// its last complete packet; a packet cut short by the end of the file is
    /// not an error, as gpg does not report it. Extra is `contains no keys`
    /// when the file reads but has no public key packet. The Location is the
    /// key file, or its target for a symlink. Native packages are skipped:
    /// lintian reports a key there under another Tag.
    ///
    /// Lintian's Check runs `gpg --list-packets` and this Rule reads the packets
    /// itself (`lint::openpgp`). Differences from lintian: lintian's further
    /// messages (a key with no key id or no public key packet of its own, and
    /// the key's own packets) are not made here, and no check is done on
    /// whether the key can verify.
    PublicUpstreamKeyUnusable,
    code = "LN1535",
    tag = "public-upstream-key-unusable",
    default_selected = false,
    experimental = false,
}

impl SourceRule for PublicUpstreamKeyUnusable {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if is_native(ctx) {
            return;
        }
        let Some(files) = ctx.signing_key_files() else {
            return;
        };
        for file in files.iter() {
            let extra = if file.key.error().is_some() {
                "cannot be processed"
            } else if file.key.public_keys().is_empty() {
                "contains no keys"
            } else {
                continue;
            };
            ctx.diagnostic(
                Diagnostic::warn(extra)
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

#[cfg(test)]
mod tests {
    use crate::lint::dsc::Dsc;
    use crate::lint::openpgp::TINY_KEY;
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

    fn package(
        format: &str,
        version: &str,
        extra: &[(&'static str, TestFile)],
    ) -> SourcePackageCase {
        let text = format!(
            "Format: {format}\nSource: example\nBinary: example\nArchitecture: all\nVersion: {version}\n\
             Maintainer: Example <ex@example.com>\nFiles:\n d41d8cd98f00b204e9800998ecf8427e 0 example_{version}.tar.xz\n"
        );
        let dsc = Dsc::parse(&text).expect("valid dsc");
        let mut files = vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/changelog", TestFile::new(entry(version))),
        ];
        files.extend(extra.iter().cloned());
        SourcePackageCase::from(files).dsc(dsc)
    }

    #[test]
    fn source_tree() {
        let pass = vec![
            tree("3.0 (quilt)", "1.0-1", &[]),
            tree("3.0 (quilt)", "1.0-1", &[(KEY, TestFile::new(TINY_KEY))]),
            // Native packages are reported under another Tag.
            tree("3.0 (native)", "1.0", &[(KEY, TestFile::new(""))]),
            // A directory is not a key file.
            tree("3.0 (quilt)", "1.0-1", &[(KEY, TestFile::directory())]),
            // A dangling symlink resolves to nothing.
            tree(
                "3.0 (quilt)",
                "1.0-1",
                &[(KEY, TestFile::symlink("nowhere"))],
            ),
            // A symlink to a good key is read through.
            tree(
                "3.0 (quilt)",
                "1.0-1",
                &[
                    (KEY, TestFile::symlink("other")),
                    ("debian/upstream/other", TestFile::new(TINY_KEY)),
                ],
            ),
        ];
        let fail = vec![
            tree("3.0 (quilt)", "1.0-1", &[(KEY, TestFile::new(""))]),
            tree(
                "3.0 (quilt)",
                "1.0-1",
                &[(
                    KEY,
                    TestFile::new("Testing an empty file would require source format 3.0\n"),
                )],
            ),
            tree(
                "3.0 (quilt)",
                "1.0-1",
                &[(KEY, TestFile::new(TINY_KEY.replace("=11jc", "=11jd")))],
            ),
            tree(
                "3.0 (quilt)",
                "1.0-1",
                &[(KEY, TestFile::from_bytes(b"\xb4\x04Test".to_vec()))],
            ),
            // A symlink is followed to its target, which is the Location.
            tree(
                "3.0 (quilt)",
                "1.0-1",
                &[
                    (KEY, TestFile::symlink("other")),
                    ("debian/upstream/other", TestFile::new("")),
                ],
            ),
            // Two key files, both unusable.
            tree(
                "3.0 (quilt)",
                "1.0-1",
                &[
                    ("debian/upstream-signing-key.pgp", TestFile::new("")),
                    (KEY, TestFile::new("nonsense\n")),
                ],
            ),
        ];
        SourceTreeTester::new(PublicUpstreamKeyUnusable, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![
            package("3.0 (quilt)", "1.0-1", &[(KEY, TestFile::new(TINY_KEY))]),
            package("3.0 (native)", "1.0", &[(KEY, TestFile::new(""))]),
        ];
        let fail = vec![package("3.0 (quilt)", "1.0-1", &[(KEY, TestFile::new(""))])];
        SourcePackageTester::new(PublicUpstreamKeyUnusable, pass, fail).test_and_snapshot();
    }
}
