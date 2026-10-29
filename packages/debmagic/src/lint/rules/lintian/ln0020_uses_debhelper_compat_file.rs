use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debhelper::{EXPERIMENTAL_COMPAT_LEVEL, VERSIONED_PREREQUISITE_AVAILABLE};
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct UsesDebhelperCompatFile;

declare_rule! {
    /// The package declares its debhelper compat level in `debian/compat`.
    ///
    /// Emitted only after `debian/rules` shows a dh command (and the
    /// `Build-Driver` is `debian-rules` or absent). The first line of a
    /// readable `debian/compat`, trimmed, is read as a number; levels from 11
    /// (when `debhelper-compat` became available) up to, but not including,
    /// the experimental level 15 are flagged. A missing or empty file, or a
    /// non-numeric level, is not. Extra is empty; `debian/compat` is the
    /// Location. Applies to the Source tree and Source package.
    UsesDebhelperCompatFile,
    code = "LN0020",
    tag = "uses-debhelper-compat-file",
    default_selected = true,
    experimental = false,
}

impl SourceRule for UsesDebhelperCompatFile {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        let Some(facts) = ctx.debhelper() else {
            return;
        };
        if !facts.uses_debhelper() {
            return;
        }
        let Some(level) = facts.compat_file_value() else {
            return;
        };
        if (VERSIONED_PREREQUISITE_AVAILABLE..EXPERIMENTAL_COMPAT_LEVEL).contains(&level) {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: PathBuf::from("debian/compat"),
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const CONTROL: &str = "\
Source: example\n\
Maintainer: Example <ex@example.com>\n\
Standards-Version: 4.7.2\n\
\n\
Package: example\n\
Architecture: all\n\
Description: example\n extra\n";

    const DH: &str = "#!/usr/bin/make -f\n%:\n\tdh $@\n";

    fn package(rules: &str, compat: Option<&str>) -> Vec<(&'static str, TestFile)> {
        let mut files = vec![
            ("debian/control", TestFile::new(CONTROL)),
            ("debian/rules", TestFile::new(rules)),
        ];
        if let Some(compat) = compat {
            files.push(("debian/compat", TestFile::new(compat)));
        }
        files
    }

    #[test]
    fn test() {
        let pass = vec![
            vec![("README", TestFile::new("no packaging"))],
            package(DH, None),
            package(DH, Some("10\n")),
            package(DH, Some("9\n")),
            package(DH, Some("15\n")),
            package(DH, Some("\n")),
            package(DH, Some("abc\n")),
            // Not a dh package.
            package("#!/usr/bin/make -f\nbuild:\n\ttrue\n", Some("13\n")),
            vec![
                (
                    "debian/control",
                    TestFile::new(
                        "Source: example\n\
                         Build-Driver: debputy\n\
                         \n\
                         Package: example\n\
                         Architecture: all\n\
                         Description: example\n extra\n",
                    ),
                ),
                ("debian/rules", TestFile::new(DH)),
                ("debian/compat", TestFile::new("13\n")),
            ],
        ];
        let fail = vec![
            package(DH, Some("11\n")),
            package(DH, Some("13\n")),
            package(DH, Some("14\n")),
            package(DH, Some("\t13  \n")),
            package(DH, Some("13\n\n12\n")),
            package(DH, Some("13abc\n")),
            // Not tied to the Build-Depends declaration or the rules level.
            vec![
                ("debian/control", TestFile::new(CONTROL)),
                ("debian/rules", TestFile::new(DH)),
                ("debian/compat", TestFile::symlink("level")),
                ("debian/level", TestFile::new("12\n")),
            ],
        ];
        SourceTreeTester::new(UsesDebhelperCompatFile, pass, fail).test_and_snapshot();
    }

    #[test]
    fn test_source_package() {
        let pass = vec![SourcePackageCase::from(package(DH, None))];
        let fail = vec![SourcePackageCase::from(package(DH, Some("13\n")))];
        SourcePackageTester::new(UsesDebhelperCompatFile, pass, fail).test_and_snapshot();
    }
}
