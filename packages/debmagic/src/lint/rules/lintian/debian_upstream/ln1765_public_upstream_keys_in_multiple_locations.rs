use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::{Severity, SourceRule};

pub struct PublicUpstreamKeysInMultipleLocations;

declare_rule! {
    /// The Source tree or Source package has more than one public upstream
    /// signing key file under `debian/`.
    ///
    /// The key files are lintian's signing key names under `debian/` (see
    /// `public-upstream-key-unusable`), counted when the name resolves to a
    /// file. Extra is the names relative to `debian/`, sorted and separated by
    /// spaces, as lintian's `hint` gives them; there is no Location, as the
    /// Subject is the location. Info and not selected by default, as in
    /// lintian. Native packages are checked too.
    ///
    /// Difference from lintian: a symlink that resolves to a key file is
    /// named by its target here, where lintian names the link.
    PublicUpstreamKeysInMultipleLocations,
    code = "LN1765",
    tag = "public-upstream-keys-in-multiple-locations",
    default_selected = false,
    experimental = false,
}

impl SourceRule for PublicUpstreamKeysInMultipleLocations {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(files) = ctx.signing_key_files() else {
            return;
        };
        if files.len() < 2 {
            return;
        }
        let mut names: Vec<String> = files
            .iter()
            .map(|file| {
                let path = file.path.to_string_lossy();
                path.strip_prefix("debian/")
                    .map_or_else(|| path.to_string(), str::to_string)
            })
            .collect();
        names.sort();
        ctx.diagnostic(Diagnostic::warn(names.join(" ")).with_severity(Severity::Info));
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

    #[test]
    fn source_tree() {
        let pass = vec![
            tree("3.0 (quilt)", "1.0-1", &[]),
            tree(
                "3.0 (quilt)",
                "1.0-1",
                &[("debian/upstream/signing-key.asc", TestFile::new(TINY_KEY))],
            ),
            // Only names that resolve to a file count.
            tree(
                "3.0 (quilt)",
                "1.0-1",
                &[
                    (
                        "debian/upstream-signing-key.pgp",
                        TestFile::symlink("nowhere"),
                    ),
                    ("debian/upstream/signing-key.asc", TestFile::new(TINY_KEY)),
                ],
            ),
            // Directories are not key files.
            tree(
                "3.0 (quilt)",
                "1.0-1",
                &[
                    ("debian/upstream-signing-key.pgp", TestFile::directory()),
                    ("debian/upstream/signing-key.asc", TestFile::new(TINY_KEY)),
                ],
            ),
        ];
        let fail = vec![
            tree(
                "3.0 (quilt)",
                "1.0-1",
                &[
                    ("debian/upstream/signing-key.asc", TestFile::new(TINY_KEY)),
                    ("debian/upstream-signing-key.pgp", TestFile::new(TINY_KEY)),
                ],
            ),
            tree(
                "3.0 (native)",
                "1.0",
                &[
                    ("debian/upstream/signing-key.asc", TestFile::new("")),
                    ("debian/upstream-signing-key.pgp", TestFile::new("")),
                ],
            ),
        ];
        SourceTreeTester::new(PublicUpstreamKeysInMultipleLocations, pass, fail)
            .test_and_snapshot();
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
    fn source_package() {
        let pass = vec![package(&[(
            "debian/upstream/signing-key.asc",
            TestFile::new(""),
        )])];
        let fail = vec![package(&[
            ("debian/upstream/signing-key.asc", TestFile::new("")),
            ("debian/upstream-signing-key.pgp", TestFile::new("")),
        ])];
        SourcePackageTester::new(PublicUpstreamKeysInMultipleLocations, pass, fail)
            .test_and_snapshot();
    }
}
