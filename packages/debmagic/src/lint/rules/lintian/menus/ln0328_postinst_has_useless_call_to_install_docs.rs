use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct PostinstHasUselessCallToInstallDocs;

declare_rule! {
    /// `postinst` calls `install-docs`.
    ///
    /// Triggers no longer need that call. `install-docs` and
    /// `/usr/sbin/install-docs` count, including `-r` and `--remove`. A
    /// `command` or `-x` probe is removed before the call is recognized, so
    /// `command install-docs` alone is not a call. `exec install-docs` is not
    /// a call. The non-remove hit is reported first, then the remove hit, each
    /// on its last line. Extra is empty. Location is `postinst`.
    PostinstHasUselessCallToInstallDocs,
    code = "LN0328",
    tag = "postinst-has-useless-call-to-install-docs",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for PostinstHasUselessCallToInstallDocs {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let calls = {
            let Some(files) = ctx.control_files() else {
                return;
            };
            scan::install_docs_calls(files, "postinst")
        };
        if let Some(line) = calls.add {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: PathBuf::from("postinst"),
                line: Some(line),
                column: None,
            }));
        }
        if let Some(line) = calls.remove {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path: PathBuf::from("postinst"),
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

    fn postinst(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("postinst", TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            postinst("#!/bin/sh\ntrue\n"),
            postinst("#!/bin/sh\n# install-docs\n"),
            postinst("#!/bin/sh\ncommand install-docs\n"),
            postinst("#!/bin/sh\nexec install-docs\n"),
            postinst("#!/bin/sh\ninstall-docs-foo\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::symlink("missing")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("prerm", TestFile::new("#!/bin/sh\ninstall-docs\n")),
        ];
        let fail = vec![
            postinst("#!/bin/sh\nset -e\n\ninstall-docs\n"),
            postinst("#!/bin/sh\n    install-docs -i /usr/share/doc-base/test-doc\n"),
            postinst("#!/bin/sh\n/usr/sbin/install-docs foo\n"),
            postinst("#!/bin/sh\ninstall-docs -r foo\ninstall-docs bar\n"),
            postinst("#!/bin/sh\ninstall-docs --remove foo\n"),
            postinst("install-docs\n"),
            postinst("#!/bin/sh\nif type install-docs >/dev/null; then install-docs foo; fi\n"),
            postinst(
                "#!/bin/sh\nif [ -x /usr/sbin/install-docs ]; then\n    install-docs foo\nfi\n",
            ),
        ];
        BinaryPackageTester::new(PostinstHasUselessCallToInstallDocs, pass, fail)
            .test_and_snapshot();
    }
}
