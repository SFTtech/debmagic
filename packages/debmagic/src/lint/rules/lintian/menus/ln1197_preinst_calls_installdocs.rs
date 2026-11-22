use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct PreinstCallsInstalldocs;

declare_rule! {
    /// `preinst` calls `install-docs`.
    ///
    /// Doc-base registration belongs to triggers, so `preinst` must not call
    /// `install-docs`. Only the adding call counts: a `-r` or `--remove` call
    /// is not reported here. The recognition rules are those of
    /// `postinst-has-useless-call-to-install-docs`. Only the last call is
    /// reported. Extra is empty. Location is `preinst` and that line.
    PreinstCallsInstalldocs,
    code = "LN1197",
    tag = "preinst-calls-installdocs",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PreinstCallsInstalldocs {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let calls = {
            let Some(files) = ctx.control_files() else {
                return;
            };
            scan::install_docs_calls(files, "preinst")
        };
        if let Some(line) = calls.add {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path: PathBuf::from("preinst"),
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

    fn preinst(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("preinst", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            preinst("#!/bin/sh\ntrue\n"),
            preinst("#!/bin/sh\n# install-docs\n"),
            preinst("#!/bin/sh\ncommand install-docs\n"),
            preinst("#!/bin/sh\nexec install-docs\n"),
            preinst("#!/bin/sh\ninstall-docs-foo\n"),
            // Remove calls are not this Tag.
            preinst("#!/bin/sh\ninstall-docs -r foo\n"),
            preinst("#!/bin/sh\ninstall-docs --remove foo\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::new("#!/bin/sh\ninstall-docs\n")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("preinst", TestFile::symlink("missing")),
        ];
        let fail = vec![
            preinst("#!/bin/sh\nset -e\n\ninstall-docs\n"),
            preinst("#!/bin/sh\n    install-docs -i /usr/share/doc-base/test-doc\n"),
            preinst("#!/bin/sh\ninstall-docs foo\ninstall-docs bar\n"),
            preinst("#!/bin/sh\n/usr/sbin/install-docs foo\n"),
            preinst("install-docs\n"),
        ];
        BinaryPackageTester::new(PreinstCallsInstalldocs, pass, fail).test_and_snapshot();
    }
}
