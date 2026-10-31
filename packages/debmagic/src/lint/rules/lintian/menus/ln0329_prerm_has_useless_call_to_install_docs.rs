use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct PrermHasUselessCallToInstallDocs;

declare_rule! {
    /// `prerm` calls `install-docs`.
    ///
    /// Same recognition as the `postinst` Tag, including a separate hit for
    /// `-r` / `--remove`. Extra is empty. Location is `prerm`.
    PrermHasUselessCallToInstallDocs,
    code = "LN0329",
    tag = "prerm-has-useless-call-to-install-docs",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PrermHasUselessCallToInstallDocs {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let calls = {
            let Some(files) = ctx.control_files() else {
                return;
            };
            scan::install_docs_calls(files, "prerm")
        };
        if let Some(line) = calls.add {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: PathBuf::from("prerm"),
                line: Some(line),
                column: None,
            }));
        }
        if let Some(line) = calls.remove {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: PathBuf::from("prerm"),
                line: Some(line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn prerm(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("prerm", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            prerm("#!/bin/sh\ntrue\n"),
            prerm("#!/bin/sh\ncommand -v install-docs\n"),
            prerm("#!/bin/sh\nexec install-docs foo\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::new("#!/bin/sh\ninstall-docs\n")),
            prerm("").control_file("prerm", TestFile::symlink("missing")),
        ];
        let fail = vec![
            prerm(
                "#!/bin/sh\nset -e\nif [ -x \"`which install-docs 2>/dev/null`\" ] ; then\n    install-docs -r test-doc\nfi\n",
            ),
            prerm(
                "#!/bin/sh\n\nif which install-docs >/dev/null 2>&1 ; then\n    install-docs\nfi\n",
            ),
            prerm("#!/bin/sh\ninstall-docs --remove foo\ninstall-docs bar\n"),
            prerm("install-docs -i foo\n"),
        ];
        BinaryPackageTester::new(PrermHasUselessCallToInstallDocs, pass, fail).test_and_snapshot();
    }
}
