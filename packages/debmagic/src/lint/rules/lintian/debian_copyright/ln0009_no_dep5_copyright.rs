use crate::declare_rule;
use crate::lint::context::SourceContext;
use crate::lint::debian_copyright::DebianCopyright;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::SourceRule;

pub struct NoDep5Copyright;

declare_rule! {
    /// A Debian copyright file is not in machine-readable (DEP-5) format.
    ///
    /// Reads `debian/copyright` and `debian/<installable>.copyright` (symlinks
    /// and files that are not UTF-8 are skipped). A file is machine-readable
    /// when a `Format` field (or `Format-Specification`) mentions DEP-5 or the
    /// copyright-format; the version of that format is not judged here. Extra
    /// is empty; the Location is the copyright file.
    NoDep5Copyright,
    code = "LN0009",
    tag = "no-dep5-copyright",
    default_selected = true,
    experimental = false,
}

impl SourceRule for NoDep5Copyright {
    fn run(&self, ctx: &mut SourceContext<'_>) {
        for (path, copyright) in ctx.debian_copyrights() {
            if copyright == DebianCopyright::NotMachineReadable {
                ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                    path,
                    line: None,
                    column: None,
                }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{SourcePackageCase, SourcePackageTester, SourceTreeTester, TestFile};

    use super::*;

    const DEP5: &str = "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n\nFiles: *\nCopyright: 2024 Someone\nLicense: MIT\n";
    const CONTROL: &str = "Source: example\n\nPackage: foo\nArchitecture: all\nDescription: foo\n";

    #[test]
    fn test() {
        let pass = vec![
            vec![("debian/copyright", DEP5)],
            Vec::new(),
            // Any copyright-format version counts, and so does an unreadable body.
            vec![(
                "debian/copyright",
                "Format: http://wiki.debian.org/Proposals/CopyrightFormat\n",
            )],
            vec![(
                "debian/copyright",
                "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\nFiles: a\nFiles: b\n",
            )],
            vec![(
                "debian/copyright",
                "Format-Specification: http://dep.debian.net/deps/dep5/\n",
            )],
            // Other files in `debian/` are not copyright files.
            vec![("debian/notes.copyright", "plain text")],
            // Not an installable.
            vec![
                ("debian/control", CONTROL),
                ("debian/copyright", DEP5),
                ("debian/bar.copyright", "plain"),
            ],
        ];
        let fail = vec![
            vec![(
                "debian/copyright",
                "Copyright 2024 Someone\nLicense: GPL-2\n",
            )],
            vec![("debian/copyright", "")],
            // Mentioning DEP-5 outside of a Format field is not enough.
            vec![(
                "debian/copyright",
                "Upstream-Name: x\nComment: see /copyright-format/\n",
            )],
            vec![
                ("debian/control", CONTROL),
                ("debian/copyright", DEP5),
                ("debian/foo.copyright", "plain"),
            ],
        ];
        SourceTreeTester::new(NoDep5Copyright, pass, fail).test_and_snapshot();
    }

    #[test]
    fn files_that_are_not_regular_or_not_utf8_are_skipped() {
        let pass = vec![
            vec![("debian/copyright", TestFile::symlink("elsewhere"))],
            vec![("debian/copyright", TestFile::directory())],
        ];
        SourceTreeTester::new(NoDep5Copyright, pass, Vec::<Vec<(&str, TestFile)>>::new()).test();
    }

    #[test]
    fn source_package() {
        let pass = vec![SourcePackageCase::from(vec![("debian/copyright", DEP5)])];
        let fail = vec![SourcePackageCase::from(vec![("debian/copyright", "plain")])];
        SourcePackageTester::new(NoDep5Copyright, pass, fail).test_and_snapshot();
    }
}
