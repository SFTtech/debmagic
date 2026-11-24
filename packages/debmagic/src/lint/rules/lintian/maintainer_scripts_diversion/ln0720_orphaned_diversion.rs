use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct OrphanedDiversion;

declare_rule! {
    /// A maintainer script adds a diversion that no maintainer script removes.
    ///
    /// One finding per added path (the last `--add` of a path wins). A
    /// `--remove` of the same path clears it, after the wildcard merge of
    /// LN0709. Any script may add; `--local` does not matter. Extra is the
    /// path without its leading `/`, as in LN0709. The Location is the last
    /// physical line of the winning `--add`.
    OrphanedDiversion,
    code = "LN0720",
    tag = "orphaned-diversion",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for OrphanedDiversion {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(diversions) = ctx.diversions() else {
            return;
        };
        for diversion in diversions.added().into_iter().filter(|d| !d.is_removed()) {
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

    fn case(scripts: &[(&str, &str)]) -> BinaryPackageCase {
        let mut case = BinaryPackageCase::from(Vec::<(&str, TestFile)>::new());
        for &(name, content) in scripts {
            case = case.control_file(name, TestFile::new(content));
        }
        case
    }

    #[test]
    fn test() {
        let pass = vec![
            case(&[]),
            case(&[
                (
                    "preinst",
                    "#!/bin/sh\ndpkg-divert --package foo --add --rename /usr/bin/foo\n",
                ),
                (
                    "postrm",
                    "#!/bin/sh\ndpkg-divert --package foo --remove --rename /usr/bin/foo\n",
                ),
            ]),
            // Removed by another script, with an option in between.
            case(&[
                (
                    "preinst",
                    "#!/bin/sh\ndpkg-divert --local --add --divert /usr/bin/foo.real \\\n  /usr/bin/foo\n",
                ),
                (
                    "postrm",
                    "#!/bin/sh\ndpkg-divert --local --remove --rename /usr/bin/foo\n",
                ),
            ]),
            // Removed in an alternative branch of the same script.
            case(&[
                (
                    "postrm",
                    "#!/bin/sh\nif [ remove = \"$1\" ]; then\n  dpkg-divert --remove /usr/bin/foo\nfi\ndpkg-divert --remove /usr/bin/foo\n",
                ),
                ("preinst", "#!/bin/sh\ndpkg-divert --add /usr/bin/foo\n"),
            ]),
            // A removal of a wider pattern covers the added path.
            case(&[
                (
                    "preinst",
                    "#!/bin/sh\ndpkg-divert --add /etc/scripts/orphan\n",
                ),
                (
                    "postrm",
                    "#!/bin/sh\ndpkg-divert --remove /etc/scripts/$f\n",
                ),
            ]),
            // A removal of a path inside an added pattern covers it, too.
            case(&[
                ("preinst", "#!/bin/sh\ndpkg-divert --add /etc/scripts/$f\n"),
                (
                    "postrm",
                    "#!/bin/sh\ndpkg-divert --remove /etc/scripts/orphan\n",
                ),
            ]),
            // No interpreter: not a maintainer script lintian reads.
            case(&[("preinst", "dpkg-divert --add /usr/bin/foo\n")]),
            // Not an addition.
            case(&[("postrm", "#!/bin/sh\ndpkg-divert --remove /usr/bin/foo\n")]),
            case(&[("preinst", "#!/bin/sh\ndpkg-divert --list /usr/bin/foo\n")]),
        ];
        let fail = vec![
            case(&[(
                "preinst",
                "#!/bin/sh\ndpkg-divert --local --add --divert /usr/bin/bar.real \\\n    /usr/bin/bar\n",
            )]),
            // The last `--add` of a path is the Location.
            case(&[(
                "preinst",
                "#!/bin/sh\ndpkg-divert --add /usr/bin/bar\ndpkg-divert --add /usr/bin/bar 2> /dev/null\n",
            )]),
            // A removal of another path does not clear it.
            case(&[
                ("preinst", "#!/bin/sh\ndpkg-divert --add /usr/bin/foo\n"),
                ("postrm", "#!/bin/sh\ndpkg-divert --remove /usr/bin/bar\n"),
            ]),
            // An added pattern with no removal of its own.
            case(&[(
                "preinst",
                "#!/bin/sh\ndpkg-divert --add /etc/scripts/$f|| true\ndpkg-divert --add /usr/share/foo/${f#f}\n",
            )]),
            // Diversions in several scripts, one of them removed.
            case(&[
                ("postinst", "#!/bin/sh\ndpkg-divert --add /usr/bin/foo\n"),
                ("preinst", "#!/bin/sh\ndpkg-divert --add /usr/bin/bar\n"),
                ("postrm", "#!/bin/sh\ndpkg-divert --remove /usr/bin/bar\n"),
            ]),
        ];
        BinaryPackageTester::new(OrphanedDiversion, pass, fail).test_and_snapshot();
    }
}
