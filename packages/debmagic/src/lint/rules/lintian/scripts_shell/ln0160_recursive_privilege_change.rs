use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct RecursivePrivilegeChange;

declare_rule! {
    /// A maintainer script changes ownership or mode recursively.
    ///
    /// The match is `chmod` or `chown` with `-R` or `--recursive`, or `find`
    /// that reaches `exec` and then `chmod` or `chown` without first passing
    /// `-maxdepth 1`. One match per logical line, the leftmost. The extra is
    /// that match in double quotes. Shell scripts, `perl`, and any interpreter
    /// on `config` or `postrm` are scanned. Applies to udebs.
    RecursivePrivilegeChange,
    code = "LN0160",
    tag = "recursive-privilege-change",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for RecursivePrivilegeChange {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        for script in scan::maintainer_scripts(files) {
            for line in script.lines {
                let Some(matched) = scan::recursive_privilege(&line.text) else {
                    continue;
                };
                ctx.diagnostic(Diagnostic::warn(format!("\"{matched}\"")).with_location(
                    Location {
                        path: script.path.clone(),
                        line: Some(line.number),
                        column: None,
                    },
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn with_script(name: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(content))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            with_script("postinst", "#!/bin/sh\n"),
            with_script("postinst", "#!/bin/sh\n# chown -R /tmp\n"),
            with_script("postinst", "#!/bin/sh\nset -e\nchown root:root /good\n"),
            with_script("postinst", "#!/bin/sh\nset -e\nchmod 777 /good\n"),
            with_script(
                "postinst",
                "#!/bin/sh\nset -e\nfind /bad -maxdepth 1 -type d -exec chown root:root {} \\;\n",
            ),
            with_script(
                "postinst",
                "#!/bin/sh\nset -e\nfind /bad -maxdepth  1 -exec chmod\n",
            ),
            // Unknown on postinst: the interpreters table is not consulted.
            with_script("postinst", "#!/usr/bin/python3\nchown -R /tmp\n"),
            with_script("postinst", "#!/usr/bin/env sh\nchown -R /tmp\n"),
            with_script("postinst", "#!/bin/pdksh\nchown -R /tmp\n"),
            with_script("triggers", "#!/bin/sh\nchown -R /tmp\n"),
            BinaryPackageCase::from(vec![(
                "usr/bin/example",
                TestFile::new("#!/bin/sh\nchown -R /tmp\n"),
            )]),
        ];
        let fail = vec![
            with_script("postinst", "#!/bin/sh\nset -e\nchown -R /bad\n"),
            with_script("postinst", "#!/bin/sh\nset -e\nchown root:root -R /bad\n"),
            with_script(
                "postinst",
                "#!/bin/sh\nset -e\nchmod 777 --recursive /bad\n",
            ),
            with_script(
                "postinst",
                "#!/bin/sh\nset -e\nfind /bad -maxdepth 2 -type d -exec chown root:root {} \\;\n",
            ),
            with_script(
                "postinst",
                "#!/bin/sh\nset -e\nfind /bad -maxdepth 2 -exec echo; chown -R /x\n",
            ),
            // Still reported inside a debhelper section, and on perl.
            with_script(
                "postinst",
                "#!/bin/sh\nset -e\n# Automatically added by dh_dummy/12\nchown -R /bad\n# End automatically added section\n",
            ),
            with_script("postinst", "#!/usr/bin/perl\nchown -R /bad\n"),
            with_script("config", "#!/usr/bin/python3\nchown -R /bad\n"),
            with_script("postinst", "#!/bin/sh\nchmod -R 755 /bad\n").udeb(),
        ];
        BinaryPackageTester::new(RecursivePrivilegeChange, pass, fail).test_and_snapshot();
    }
}
