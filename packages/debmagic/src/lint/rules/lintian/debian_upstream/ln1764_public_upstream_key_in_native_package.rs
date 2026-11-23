use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{Severity, SourceRule};
use crate::lint::rules::lintian::fields_version::is_native;

pub struct PublicUpstreamKeyInNativePackage;

declare_rule! {
    /// A native package ships a public upstream signing key under `debian/`.
    ///
    /// The key files are lintian's signing key names under `debian/`, as read
    /// by `public-upstream-key-unusable`: a name that does not resolve to a
    /// file (symlinks are followed to their target) is not reported. Each key
    /// file of a native package is reported once, with its path as Location
    /// and no extra; the key is not parsed, so an unusable key is reported
    /// here too. Info and not selected by default, as in lintian.
    PublicUpstreamKeyInNativePackage,
    code = "LN1764",
    tag = "public-upstream-key-in-native-package",
    default_selected = false,
    experimental = false,
}

impl SourceRule for PublicUpstreamKeyInNativePackage {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        if !is_native(ctx) {
            return;
        }
        let Some(files) = ctx.signing_key_files() else {
            return;
        };
        for file in files.iter() {
            ctx.diagnostic(
                Diagnostic::warn("")
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
            tree("3.0 (native)", "1.0", &[]),
            // Not native: reported under the other Tags.
            tree("3.0 (quilt)", "1.0-1", &[(KEY, TestFile::new(TINY_KEY))]),
            // A dangling symlink is not a key file.
            tree(
                "3.0 (native)",
                "1.0",
                &[(KEY, TestFile::symlink("nowhere"))],
            ),
            // A directory is not a key file.
            tree("3.0 (native)", "1.0", &[(KEY, TestFile::directory())]),
        ];
        let fail = vec![
            tree("3.0 (native)", "1.0", &[(KEY, TestFile::new(TINY_KEY))]),
            tree("3.0 (native)", "1.0", &[(KEY, TestFile::new(""))]),
            tree(
                "3.0 (native)",
                "1.0",
                &[
                    (KEY, TestFile::symlink("other")),
                    ("debian/upstream/other", TestFile::new(TINY_KEY)),
                ],
            ),
        ];
        SourceTreeTester::new(PublicUpstreamKeyInNativePackage, pass, fail).test_and_snapshot();
    }

    #[test]
    fn source_package() {
        let pass = vec![package(
            "3.0 (quilt)",
            "1.0-1",
            &[(KEY, TestFile::new(TINY_KEY))],
        )];
        let fail = vec![
            package("3.0 (native)", "1.0", &[(KEY, TestFile::new(TINY_KEY))]),
            package("3.0 (native)", "1.0", &[(KEY, TestFile::new(""))]),
        ];
        SourcePackageTester::new(PublicUpstreamKeyInNativePackage, pass, fail).test_and_snapshot();
    }
}
