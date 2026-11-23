use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::{BinaryPackageRule, Severity};
use crate::lint::spelling::{check_spelling, group_exceptions};

use super::readme::readme_text;

pub struct SpellingErrorInReadmeDebian;

declare_rule! {
    /// The `README.Debian` of the package contains a misspelling from
    /// lintian's spelling lists.
    ///
    /// The first readable file of `README.Debian.gz`, `README.Debian`,
    /// `README.debian.gz`, and `README.debian` under `usr/share/doc/<package>/`
    /// is checked as a whole, with duplicate words and the spelling exceptions
    /// of the Binary package's group, as lintian's `check_spelling` does. The
    /// Location is that file with no line; extra is the word and its
    /// correction. Pedantic and not selected by default, as in lintian. Udebs
    /// are checked too.
    SpellingErrorInReadmeDebian,
    code = "LN1760",
    tag = "spelling-error-in-readme-debian",
    default_selected = false,
    experimental = false,
}

impl BinaryPackageRule for SpellingErrorInReadmeDebian {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let Some((path, text)) = ({
            let Some(files) = ctx.data_files() else {
                return;
            };
            readme_text(files, &package)
        }) else {
            return;
        };
        let exceptions = group_exceptions(ctx);
        for (found, correction) in check_spelling(&text, &exceptions, true) {
            ctx.diagnostic(
                Diagnostic::warn(format!("{found} {correction}"))
                    .with_location(Location {
                        path: path.clone(),
                        line: None,
                        column: None,
                    })
                    .with_severity(Severity::Pedantic),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn readme(package: &str, name: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(
            format!("usr/share/doc/{package}/{name}"),
            TestFile::new(body),
        )])
        .package(package)
    }

    fn gzip_readme(package: &str, body: &str) -> BinaryPackageCase {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(body.as_bytes()).expect("gzip");
        BinaryPackageCase::from(vec![(
            format!("usr/share/doc/{package}/README.Debian.gz"),
            TestFile::from_bytes(encoder.finish().expect("gzip finish")),
        )])
        .package(package)
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            readme("example", "README.Debian", "A correct sentence here.\n"),
            // Exceptions of the package name are not reported.
            readme("teh", "README.Debian", "teh package\n"),
            // Not a readable README name.
            readme("example", "README", "teh the\n"),
            // Symlinks are not opened, as in lintian.
            BinaryPackageCase::from(vec![(
                "usr/share/doc/example/README.Debian",
                TestFile::symlink("missing"),
            )]),
        ];
        let fail = vec![
            readme("example", "README.Debian", "This has a speling error.\n"),
            readme("example", "README.Debian", "A word word repeated.\n"),
            gzip_readme("example", "Gzipped speling.\n"),
            readme("example", "README.Debian", "A speling error.\n").udeb(),
        ];
        BinaryPackageTester::new(SpellingErrorInReadmeDebian, pass, fail).test_and_snapshot();
    }
}
