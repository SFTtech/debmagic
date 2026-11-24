use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct RemoveOfUnknownDiversion;

declare_rule! {
    /// A `postrm` removes a diversion that no maintainer script adds.
    ///
    /// Only `postrm` is reported; `preinst` and `postinst` may clean up after
    /// earlier versions. A removal is known when an added diversion of the same
    /// path survives the merge, so a wider pattern covers it. Extra is the path
    /// without its leading `/`, each shell expansion as `*` in wildcard mode.
    /// The Location is the last physical line of the `--remove`.
    RemoveOfUnknownDiversion,
    code = "LN0723",
    tag = "remove-of-unknown-diversion",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for RemoveOfUnknownDiversion {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(diversions) = ctx.diversions() else {
            return;
        };
        for removal in diversions.removed() {
            if removal.is_added() || removal.script().file_name() != Some("postrm".as_ref()) {
                continue;
            }
            ctx.diagnostic(Diagnostic::error(removal.path()).with_location(Location {
                path: removal.script().to_path_buf(),
                line: Some(removal.line()),
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
            // Added in preinst, removed in postrm.
            case(
                &["usr/bin/foo"],
                &[
                    (
                        "preinst",
                        "#!/bin/sh\ndpkg-divert --package foo --add --rename \\\n  --divert /usr/bin/foo.real \\\n  /usr/bin/foo\n",
                    ),
                    (
                        "postrm",
                        "#!/bin/sh\ndpkg-divert --package foo --remove --rename /usr/bin/foo\n",
                    ),
                ],
            ),
            // Only postrm is reported; preinst and postinst may clean up.
            case(
                &[],
                &[("preinst", "#!/bin/sh\ndpkg-divert --remove /usr/bin/foo\n")],
            ),
            case(
                &[],
                &[("postinst", "#!/bin/sh\ndpkg-divert --remove /usr/bin/foo\n")],
            ),
            // Not a diversion call.
            case(
                &[],
                &[("postrm", "#!/bin/sh\ndpkg-divert --list /usr/bin/foo\n")],
            ),
            // No interpreter: not a maintainer script lintian reads.
            case(&[], &[("postrm", "dpkg-divert --remove /usr/bin/foo\n")]),
            // A wider added pattern covers the removed path.
            case(
                &["usr/share/scripts/t1/orphan"],
                &[
                    (
                        "preinst",
                        "#!/bin/sh\ndpkg-divert --add /usr/share/scripts/t1/$f\n",
                    ),
                    (
                        "postrm",
                        "#!/bin/sh\ndpkg-divert --remove /usr/share/scripts/t1/orphan\n",
                    ),
                ],
            ),
        ];
        let fail = vec![
            case(
                &[],
                &[("postrm", "#!/bin/sh\ndpkg-divert --remove /usr/bin/foo\n")],
            ),
            // Every removal line is a finding, with the last line as Location.
            case(
                &[],
                &[(
                    "postrm",
                    "#!/bin/sh\ndpkg-divert --remove /usr/bin/bar\ndpkg-divert --remove \\\n  --divert /usr/bin/bar.real /usr/bin/bar\n",
                )],
            ),
            // The removal's path is widened by a shell expansion.
            case(
                &[],
                &[(
                    "postrm",
                    "#!/bin/sh\nd=lib\ndpkg-divert --remove /usr/$d/old-file\n",
                )],
            ),
            // A removal of a path that an added diversion only partly matches.
            case(
                &[],
                &[
                    (
                        "preinst",
                        "#!/bin/sh\ndpkg-divert --add /etc/scripts/orphan\n",
                    ),
                    (
                        "postrm",
                        "#!/bin/sh\ndpkg-divert --remove /etc/scripts/other\n",
                    ),
                ],
            ),
        ];
        BinaryPackageTester::new(RemoveOfUnknownDiversion, pass, fail).test_and_snapshot();
    }
}
