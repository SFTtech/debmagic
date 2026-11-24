use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::ln0963_maintainer_script_does_not_check_for_existence_of_updatemenus::{
    Command, unchecked_calls,
};

pub struct MaintainerScriptDoesNotCheckForExistenceOfInstalldocs;

declare_rule! {
    /// A maintainer script runs `install-docs` without first checking that it
    /// exists.
    ///
    /// Scripts are `preinst`, `postinst`, `prerm` and `postrm`; the same
    /// reading and skipping as `maintainer-script-does-not-check-for-existence-of-updatemenus`
    /// applies. A call is `install-docs` or `/usr/sbin/install-docs` at the
    /// start of a line, after `;`, `&`, `|`, `then` or `do`. A check is
    /// `-x ... install-docs`, `which install-docs`, `type install-docs` or
    /// `command ... install-docs`, on the same line or an earlier one; the
    /// probe is removed from the line before the call is looked for. Only the
    /// first unchecked call in each script is reported, at its line. Extra is
    /// empty. Location is the script with its line.
    MaintainerScriptDoesNotCheckForExistenceOfInstalldocs,
    code = "LN0962",
    tag = "maintainer-script-does-not-check-for-existence-of-installdocs",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MaintainerScriptDoesNotCheckForExistenceOfInstalldocs {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package = ctx.package_name().to_string();
        let calls = {
            let Some(files) = ctx.control_files() else {
                return;
            };
            unchecked_calls(files, &package, Command::InstallDocs)
        };
        for (path, line) in calls {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
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

    fn control(name: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(body))
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            control("postinst", "#!/bin/sh\ntrue\n"),
            control("postinst", "#!/bin/sh\n# install-docs\n"),
            // Checked on the same line, or by `command`.
            control(
                "postinst",
                "#!/bin/sh\nwhich install-docs >/dev/null && install-docs -i x\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\ncommand -v install-docs >/dev/null\ninstall-docs -i x\n",
            ),
            control("postinst", "#!/bin/sh\nexec install-docs\n"),
            control("postinst", "#!/bin/sh\ninstall-docs-foo\n"),
            // A symlink is not read.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::symlink("missing")),
        ];
        let fail = vec![
            control("postinst", "#!/bin/sh\nset -e\n\ninstall-docs -i x\n"),
            control("postinst", "#!/bin/sh\n    install-docs -i x\n"),
            control("prerm", "#!/bin/sh\ninstall-docs -r x\n"),
            control("postrm", "#!/bin/sh\n/usr/sbin/install-docs -r x\n"),
        ];
        BinaryPackageTester::new(
            MaintainerScriptDoesNotCheckForExistenceOfInstalldocs,
            pass,
            fail,
        )
        .test_and_snapshot();
    }
}
