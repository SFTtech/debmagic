use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct DiversionForUnknownFile;

declare_rule! {
    /// A maintainer script adds a diversion for a path the Data files do not
    /// have.
    ///
    /// One finding per added path (the last `--add` of a path wins), whether or
    /// not a script removes it again. Extra is the path without its leading
    /// `/`. When any diversion names a shell variable or `$(…)`, each
    /// expansion is `*` in the extra and paths match Entry names as unanchored
    /// patterns; a man page matches every page of its numeric section. The
    /// Location is the last physical line of the winning `--add`.
    DiversionForUnknownFile,
    code = "LN0709",
    tag = "diversion-for-unknown-file",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for DiversionForUnknownFile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(diversions) = ctx.diversions() else {
            return;
        };
        let added = diversions.added();
        if added.is_empty() {
            return;
        }
        let Some(data) = ctx.data_files() else {
            return;
        };
        let unknown: Vec<_> = added
            .into_iter()
            .filter(|diversion| !diversion.is_in(data))
            .collect();
        for diversion in unknown {
            ctx.diagnostic(Diagnostic::error(diversion.path()).with_location(Location {
                path: diversion.script().to_path_buf(),
                line: Some(diversion.line()),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(data: &[&str], scripts: &[(&str, &str)]) -> BinaryPackageCase {
        let mut case = BinaryPackageCase::from(
            data.iter()
                .map(|path| (*path, TestFile::new("")))
                .collect::<Vec<_>>(),
        );
        for &(name, content) in scripts {
            case = case.control_file(name, TestFile::new(content));
        }
        case
    }

    #[test]
    fn test() {
        let pass = vec![
            case(&[], &[]),
            case(
                &["usr/bin/foo"],
                &[(
                    "preinst",
                    "#!/bin/sh\ndpkg-divert --package foo --add --rename \\\n  --divert /usr/bin/foo.real \\\n  /usr/bin/foo\n",
                )],
            ),
            // No interpreter: not a maintainer script lintian reads.
            case(&[], &[("preinst", "dpkg-divert --add /usr/bin/foo\n")]),
            // Removing an unknown diversion is another Tag.
            case(
                &[],
                &[("postrm", "#!/bin/sh\ndpkg-divert --remove /usr/bin/foo\n")],
            ),
            case(
                &[],
                &[("preinst", "#!/bin/sh\ndpkg-divert --list /usr/bin/foo\n")],
            ),
            case(
                &["usr/share/scripts/t1/orphan"],
                &[(
                    "preinst",
                    "#!/bin/sh\ndpkg-divert --add /usr/share/scripts/t1/\"$f\"\n",
                )],
            ),
            case(
                &["usr/share/man/man1/foo.1p.gz"],
                &[(
                    "preinst",
                    "#!/bin/sh\ndpkg-divert --add /usr/share/man/man1/foo.1.gz\n",
                )],
            ),
        ];
        let fail = vec![
            case(
                &["usr/bin/foo"],
                &[(
                    "preinst",
                    "#!/bin/sh\ndpkg-divert --local --add --divert /usr/bin/bar.real \\\n    /usr/bin/bar\n",
                )],
            ),
            // The last `--add` of a path is the Location.
            case(
                &[],
                &[(
                    "preinst",
                    "#!/bin/sh\ndpkg-divert --add /usr/bin/bar\ndpkg-divert --add /usr/bin/bar 2> /dev/null\n",
                )],
            ),
            case(
                &["usr/share/scripts/t1/orphan"],
                &[(
                    "preinst",
                    "#!/bin/sh\ndpkg-divert --add /etc/scripts/$f|| true\ndpkg-divert --add /usr/share/foo/${f#f}\n",
                )],
            ),
            // In wildcard mode a removed pattern that is wider takes over.
            case(
                &[],
                &[
                    (
                        "preinst",
                        "#!/bin/sh\ndpkg-divert --add /etc/scripts/orphan\n",
                    ),
                    (
                        "postrm",
                        "#!/bin/sh\ndpkg-divert --remove /etc/scripts/$f\n",
                    ),
                ],
            ),
        ];
        BinaryPackageTester::new(DiversionForUnknownFile, pass, fail).test_and_snapshot();
    }
}
