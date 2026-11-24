use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::copyright_text;

pub struct CopyrightFileLacksPointerToPerlLicense;

declare_rule! {
    /// The copyright file says the work is licensed as Perl itself, without
    /// pointing at `/usr/share/common-licenses`.
    ///
    /// On a Deb822 file, the license name must be `perl` or `artistic` (this
    /// match is case-sensitive) and the license body must say `as Perl
    /// itself` without `usr/share/common-licenses/`. A file that is not Deb822
    /// is matched as a whole. Udebs are not checked. Extra and Location are
    /// empty.
    CopyrightFileLacksPointerToPerlLicense,
    code = "LN0280",
    tag = "copyright-file-lacks-pointer-to-perl-license",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CopyrightFileLacksPointerToPerlLicense {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(contents) = copyright_text::installed_text(ctx) else {
            return;
        };
        if copyright_text::lacks_perl_license_pointer(&contents) {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    const DEP5: &str =
        "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n";

    fn installed(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("usr/share/doc/example", TestFile::directory()),
            ("usr/share/doc/example/copyright", TestFile::new(body)),
        ]
    }

    fn gzip(body: &str) -> Vec<u8> {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(body.as_bytes()).expect("gzip");
        encoder.finish().expect("gzip")
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(installed(
                "This program is under the same terms as Perl itself.\nSee usr/share/common-licenses/Artistic.\n",
            )),
            BinaryPackageCase::from(installed("Copyright 2024 Someone. Written in Perl.\n")),
            // The license name is case-sensitive: `Perl` is not `perl`.
            BinaryPackageCase::from(installed(&format!(
                "{DEP5}\nLicense: Perl\n This is under the same terms as Perl itself.\n"
            ))),
            BinaryPackageCase::from(vec![
                ("usr/share/doc/example", TestFile::directory()),
                (
                    "usr/share/doc/example/copyright",
                    TestFile::from_bytes(vec![0xff, 0xfe]),
                ),
            ]),
            BinaryPackageCase::from(vec![("usr/share/doc/example", TestFile::symlink("other"))]),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(installed("under the same terms as Perl itself\n")).udeb(),
        ];
        let fail = vec![
            BinaryPackageCase::from(installed(
                "The module is licensed under the same terms as Perl itself.\n",
            )),
            BinaryPackageCase::from(installed(&format!(
                "{DEP5}\nLicense: perl\n This module is under the same terms as Perl itself.\n"
            ))),
            BinaryPackageCase::from(installed(&format!(
                "{DEP5}\nLicense: artistic\n same terms as Perl itself\n"
            ))),
            BinaryPackageCase::from(vec![
                ("usr/share/doc/example", TestFile::directory()),
                (
                    "usr/share/doc/example/copyright.gz",
                    TestFile::from_bytes(gzip("under the same terms as Perl itself\n")),
                ),
            ]),
        ];
        BinaryPackageTester::new(CopyrightFileLacksPointerToPerlLicense, pass, fail)
            .test_and_snapshot();
    }
}
