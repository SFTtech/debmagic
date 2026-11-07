use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script::{bad_lsb_lines, registered_names};

pub struct InitDScriptHasBadLsbLine;

declare_rule! {
    /// A line in an init script's LSB section is not a keyword or a
    /// `Description` continuation.
    ///
    /// Direct regular files and hardlinks under `etc/init.d`. `README`,
    /// `skeleton`, `rc`, and `rcS` are skipped unless `postinst` registers
    /// them. Only the first `### BEGIN INIT INFO` section is read. Extra is
    /// empty. Location is the script and the line. Udebs are checked too.
    InitDScriptHasBadLsbLine,
    code = "LN0440",
    tag = "init.d-script-has-bad-lsb-line",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptHasBadLsbLine {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let registered = ctx
            .control_files()
            .map(registered_names)
            .unwrap_or_default();
        let Some(files) = ctx.data_files() else {
            return;
        };
        let hits = bad_lsb_lines(files, &registered);
        for (path, line) in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
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

    const BROKEN: &str = concat!(
        "#! /bin/sh\n",
        "### BEGIN INIT INFO\n",
        "# Provides:          bad-lsb\n",
        "#  Required-Start:    $local_fs $remote_fs\n",
        "# Required-Stop:\n",
        "# Default-Start:     1 2 3 4 5\n",
        "# Default-Stop:      S 0 1 6 X\n",
        "# Short-Description: Example Lintian initscript\n",
        "#                    but this can't be continued\n",
        "# Description:       An example of a bad LSB section in an init script.\n",
        "#                    This continuation is allowed (with spaces).\n",
        "#\t\t\tThis one is too (with tabs).\n",
        "# X-Debian-Foo:      Some unknown but valid keyword.\n",
        "# Foo:               Some invalid keyword.\n",
        "\n",
        "# Whoops, no terminating line.\n",
        "### BEGIN INIT INFO\n",
        "# Required-Start:    This one doesn't count.\n",
        "### END INIT INFO\n",
    );

    const GOOD: &str = concat!(
        "### BEGIN INIT INFO\n",
        "# Provides: foo\n",
        "# Description: a service\n",
        "#  continued\n",
        "### END INIT INFO\n",
    );

    fn script(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![("etc/init.d/example", TestFile::new(body))])
    }

    #[test]
    fn test() {
        let pass = vec![
            script(GOOD),
            BinaryPackageCase::from(vec![("etc/init.d/README", TestFile::new(BROKEN))]),
            script("#!/bin/sh\necho hi\n"),
        ];
        let fail = vec![script(BROKEN), script(BROKEN).udeb()];
        BinaryPackageTester::new(InitDScriptHasBadLsbLine, pass, fail).test_and_snapshot();
    }
}
