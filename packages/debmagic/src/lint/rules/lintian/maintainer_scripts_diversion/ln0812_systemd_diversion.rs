use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct SystemdDiversion;

declare_rule! {
    /// A maintainer script adds a diversion for a systemd configuration file.
    ///
    /// One finding per added path, as LN0720 (a path that a later `--remove`
    /// clears is still reported). The path, without its leading `/`, starts
    /// with `usr/lib/` or `lib/` followed by `systemd`, `udev`, `tmpfiles.d`,
    /// `sysusers.d`, `repart.d` or `modules-load.d`, with no boundary after the
    /// name. Extra is the path as LN0709 prints it. The Location is the last
    /// physical line of the winning `--add`. Udebs are checked too.
    SystemdDiversion,
    code = "LN0812",
    tag = "systemd-diversion",
    default_selected = true,
    experimental = false,
}

static SYSTEMD_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:usr/)?lib/(?:systemd|udev|tmpfiles.d|sysusers.d|repart.d|modules-load.d)")
        .expect("valid regex")
});

impl BinaryPackageRule for SystemdDiversion {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(diversions) = ctx.diversions() else {
            return;
        };
        for diversion in diversions
            .added()
            .into_iter()
            .filter(|diversion| SYSTEMD_PATH.is_match(&diversion.path()))
        {
            ctx.diagnostic(Diagnostic::error(diversion.path()).with_location(Location {
                path: diversion.script().to_path_buf(),
                line: Some(diversion.line()),
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn case(scripts: &[(&str, &str)]) -> BinaryPackageCase {
        let mut case = BinaryPackageCase::from(Vec::<(&str, TestFile)>::new());
        for &(name, content) in scripts {
            case = case.control_file(name, TestFile::new(content));
        }
        case
    }

    #[test]
    fn test() {
        let pass = vec![
            case(&[]),
            case(&[(
                "preinst",
                "#!/bin/sh\ndpkg-divert --add --rename /usr/share/foo/bar\n",
            )]),
            case(&[(
                "preinst",
                "#!/bin/sh\ndpkg-divert --add --rename /usr/lib/sysusers/x\n",
            )]),
            case(&[(
                "postrm",
                "#!/bin/sh\ndpkg-divert --remove --rename /usr/lib/systemd/x\n",
            )]),
            case(&[(
                "preinst",
                "#!/bin/sh\ndpkg-divert --add --rename /usr/lib/x/systemd\n",
            )]),
        ];
        let fail = vec![
            case(&[(
                "preinst",
                "#!/bin/sh\ndpkg-divert --add --rename /usr/lib/systemd/system/foo.service\n",
            )]),
            case(&[(
                "preinst",
                "#!/bin/sh\ndpkg-divert --add /lib/udev/rules.d/00-rule.rules\n",
            )]),
            case(&[(
                "postinst",
                "#!/bin/sh\nset -e\ndpkg-divert --add /usr/lib/tmpfiles.d/x.conf\n",
            )]),
            case(&[(
                "preinst",
                "#!/bin/sh\ndpkg-divert --add /usr/lib/sysusers.d/x\n",
            )]),
            case(&[(
                "preinst",
                "#!/bin/sh\ndpkg-divert --add /usr/lib/repart.d/x\n",
            )]),
            case(&[(
                "preinst",
                "#!/bin/sh\ndpkg-divert --add /usr/lib/modules-load.d/x.conf\n",
            )]),
            case(&[(
                "preinst",
                "#!/bin/sh\ndpkg-divert --add /lib/systemd/system/x\ndpkg-divert --remove /lib/systemd/system/x\n",
            )]),
            case(&[(
                "preinst",
                "#!/bin/sh\ndpkg-divert --add /usr/lib/systemd/system/foo.service\n",
            )])
            .udeb(),
        ];
        BinaryPackageTester::new(SystemdDiversion, pass, fail).test_and_snapshot();
    }
}
