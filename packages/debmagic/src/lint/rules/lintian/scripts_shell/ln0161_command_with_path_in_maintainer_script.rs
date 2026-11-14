use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct CommandWithPathInMaintainerScript;

declare_rule! {
    /// A shell maintainer script runs a command by absolute path.
    ///
    /// Paths under `/bin`, `/sbin`, `/usr/bin`, and `/usr/sbin` are reported
    /// when they are the command: inside backticks, in `[ -x /path ]` (or
    /// `[ ! -x /path ]`) test syntax, then as a plain command. A path that is
    /// only an argument is left alone. Lines in a debhelper automatic section
    /// are skipped; the section stays open because the end marker is compared
    /// to the raw line, newline included. Extras are the path, then
    /// `(in backticks)`, `(in test syntax)`, or `(plain script)`. Location is
    /// the Control file and the physical line where the logical line ends.
    /// Applies to udebs.
    CommandWithPathInMaintainerScript,
    code = "LN0161",
    tag = "command-with-path-in-maintainer-script",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CommandWithPathInMaintainerScript {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        for script in scan::maintainer_scripts(files) {
            if !script.is_shell {
                continue;
            }
            for line in script.lines {
                if line.in_automatic_section {
                    continue;
                }
                for message in scan::command_paths(&line.text) {
                    ctx.diagnostic(Diagnostic::warn(message).with_location(Location {
                        path: script.path.clone(),
                        line: Some(line.number),
                        column: None,
                    }));
                }
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
            with_script("postinst", "#!/bin/sh\n# /usr/bin/foo\n"),
            // A case label and an argument are not commands.
            with_script(
                "postinst",
                "#!/bin/sh\nset -e\n        /usr/bin/foo) foobarbaz;;\n",
            ),
            with_script(
                "postinst",
                "#!/bin/sh\nset -e\nstart-stop-daemon --start --quiet --name foo --startas /usr/bin/foo\n",
            ),
            // Inside the automatic section, and still after its end marker.
            with_script(
                "postinst",
                "#!/bin/sh\nset -e\n# Automatically added by dh_dummy/12\n/usr/bin/false-positive\n# End automatically added section\n/usr/bin/still-inside\n",
            ),
            with_script("postinst", "#!/usr/bin/perl\n/bin/foo\n"),
            with_script("postinst", "#!/usr/bin/env sh\n/bin/foo\n"),
            with_script("triggers", "#!/bin/sh\n/bin/foo\n"),
            BinaryPackageCase::from(vec![(
                "usr/bin/example",
                TestFile::new("#!/bin/sh\n/bin/foo\n"),
            )]),
        ];
        let fail = vec![
            with_script("postinst", "#!/bin/sh\nset -e\n/usr/bin/baz; echo bar\n"),
            with_script(
                "postinst",
                "#!/bin/sh\nset -e\ntrue `/usr/bin/foo \"$0\"`\n",
            ),
            with_script(
                "postinst",
                "#!/bin/sh\nset -e\ntrue `basename \"$0\"` `/usr/bin/foo bar`\n",
            ),
            with_script(
                "preinst",
                "#!/bin/sh\n/bin/grep -E --mmap \"^Package: foo$\" /var/lib/dpkg/status\n",
            ),
            with_script(
                "postinst",
                "#!/bin/sh\nset -e\nDIVERSIONS=`env LC_ALL=C /usr/sbin/dpkg-divert --list` || true\n",
            ),
            // The `[ -x … ]` is reported on the continuation's last line.
            with_script(
                "postinst",
                "#!/bin/sh\nset -e\nif [ ! -x /usr/sbin/dpkg-state-override ] || \\\n   true\n",
            ),
            // Only the first plain path. Backticks are all reported, before plain.
            with_script("postinst", "#!/bin/sh\nset -e\n/bin/foo; /usr/bin/bar\n"),
            with_script(
                "postinst",
                "#!/bin/sh\nset -e\ntrue ` /usr/bin/foo ` `/bin/bar; echo`\n",
            ),
            // A command before the automatic section is reported; the one inside is not.
            with_script(
                "postinst",
                "#!/bin/sh\nset -e\n/usr/bin/before\n# Automatically added by dh_dummy/12\n/usr/bin/inside\n# End automatically added section\n/usr/bin/after\n",
            ),
            with_script("postinst", "#!/bin/sh\n/bin/foo\n").udeb(),
        ];
        BinaryPackageTester::new(CommandWithPathInMaintainerScript, pass, fail).test_and_snapshot();
    }
}
