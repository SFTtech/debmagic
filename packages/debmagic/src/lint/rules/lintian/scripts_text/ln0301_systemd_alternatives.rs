use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan::{self, CommandPattern};

pub struct SystemdAlternatives;

declare_rule! {
    /// A maintainer script installs an alternative for a systemd path.
    ///
    /// `update-alternatives --install` onto `/lib` or `/usr/lib` followed by
    /// `systemd`, `udev`, `tmpfiles.d`, `sysusers.d`, `repart.d`, or
    /// `modules-load.d`. Any shebang is scanned. A shell `\` continuation is
    /// joined and the Location is its last physical line. Extra is the match
    /// in double quotes. The dot in those directory names matches any
    /// character, as in lintian's pattern.
    SystemdAlternatives,
    code = "LN0301",
    tag = "systemd-alternatives",
    default_selected = true,
    experimental = false,
}

static COMMAND: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"update-alternatives\s+--install\s+(?:/usr)?/lib/(?:systemd|udev|tmpfiles.d|sysusers.d|repart.d|modules-load.d)",
    )
    .expect("valid regex")
});

const KIND: CommandPattern = CommandPattern {
    shell_only: false,
    skip_automatic: false,
    quote: true,
};

impl BinaryPackageRule for SystemdAlternatives {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        for hit in scan::find_commands(files, &COMMAND, &KIND) {
            ctx.diagnostic(Diagnostic::error(hit.message).with_location(Location {
                path: hit.path,
                line: Some(hit.line),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn control(name: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(content))
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            control("postinst", "#!/bin/sh\nset -e\n"),
            control(
                "postinst",
                "#!/bin/sh\nupdate-alternatives --install /usr/bin/foo foo /usr/bin/bar 40\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\nupdate-alternatives --install /etc/systemd/system/foo.service foo /bin/foo 40\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\n# update-alternatives --install /lib/systemd/system/foo.service foo /bin/foo 40\n",
            ),
            control(
                "postinst",
                "#!/usr/bin/perl\nupdate-alternatives \\\n--install /lib/systemd/system/foo.service foo /bin/foo 40\n",
            ),
            control(
                "postinst",
                "update-alternatives --install /lib/systemd/system/foo.service foo /bin/foo 40\n",
            ),
            control(
                "triggers",
                "#!/bin/sh\nupdate-alternatives --install /lib/systemd/system/foo.service foo /bin/foo 40\n",
            ),
        ];
        let fail = vec![
            control(
                "postinst",
                "#!/bin/sh\nupdate-alternatives --install /lib/systemd/system/foo.service foo.service /lib/systemd/system/foo-bar.service 40\nupdate-alternatives --install /lib/udev/rules.d/foo.rules foo.rules /lib/udev/rules.d/bar.rules 40\nupdate-alternatives --install /usr/lib/tmpfiles.d/foo.conf foo.conf /usr/lib/tmpfiles.d/bar.conf 40\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\nupdate-alternatives \\\n    --install /usr/lib/systemd/system/bar.service bar.service /usr/lib/systemd/system/bar-foo.service 40\n",
            ),
            control(
                "postinst",
                "#!/bin/sh\nupdate-alternatives --install /lib/sysusers.d/foo.conf foo /bin/foo 10\nupdate-alternatives --install /usr/lib/repart.d/foo.conf foo /bin/foo 10\nupdate-alternatives --install /lib/modules-load.d/foo.conf foo /bin/foo 10\n",
            ),
            control(
                "postinst",
                "#!/usr/bin/perl\nupdate-alternatives --install /lib/systemd/system/foo.service foo /bin/foo 40\n",
            ),
            control(
                "prerm",
                "#!/bin/sh\n# Automatically added by dh_dummy/12\nupdate-alternatives --install /lib/udev/rules.d/foo.rules foo /bin/foo 40\n# End automatically added section\n",
            ),
        ];
        BinaryPackageTester::new(SystemdAlternatives, pass, fail).test_and_snapshot();
    }
}
