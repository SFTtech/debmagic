use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct PostrmCallsInstalldocs;

declare_rule! {
    /// `postrm` calls `install-docs`.
    ///
    /// Doc-base registration is handled by triggers, so `postrm` has no
    /// business calling `install-docs`, with or without `-r` and `--remove`.
    /// The recognition rules are those of `postinst-has-useless-call-to-install-docs`.
    /// The non-remove call is reported first, then the remove call, each on
    /// its last line. Extra is empty. Location is `postrm`.
    PostrmCallsInstalldocs,
    code = "LN1195",
    tag = "postrm-calls-installdocs",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PostrmCallsInstalldocs {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let calls = {
            let Some(files) = ctx.control_files() else {
                return;
            };
            scan::install_docs_calls(files, "postrm")
        };
        for line in [calls.add, calls.remove].into_iter().flatten() {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path: PathBuf::from("postrm"),
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

    fn postrm(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("postrm", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            postrm("#!/bin/sh\ntrue\n"),
            postrm("#!/bin/sh\n# install-docs\n"),
            postrm("#!/bin/sh\ncommand install-docs\n"),
            postrm("#!/bin/sh\nexec install-docs\n"),
            postrm("#!/bin/sh\ninstall-docs-foo\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::new("#!/bin/sh\ninstall-docs\n")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postrm", TestFile::symlink("missing")),
        ];
        let fail = vec![
            postrm("#!/bin/sh\nset -e\n\ninstall-docs\n"),
            postrm("#!/bin/sh\ninstall-docs -r foo\ninstall-docs bar\n"),
            postrm("#!/bin/sh\ninstall-docs --remove foo\n"),
            postrm("#!/bin/sh\ninstall-docs -r foo\n/usr/sbin/install-docs --remove bar\n"),
            postrm("install-docs\n"),
        ];
        BinaryPackageTester::new(PostrmCallsInstalldocs, pass, fail).test_and_snapshot();
    }
}
