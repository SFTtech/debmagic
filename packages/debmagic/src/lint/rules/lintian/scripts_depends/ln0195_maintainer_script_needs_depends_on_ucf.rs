use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::depends::{CommandNeed, flag_missing_dependency};

pub struct MaintainerScriptNeedsDependsOnUcf;

declare_rule! {
    /// A maintainer script calls `ucf` without a dependency on `ucf`.
    ///
    /// The call is that command followed by whitespace, outside `postrm`. A line
    /// that tests `-x`, runs `which` or `type`, runs `command`, or contains
    /// `|| true` is exempt. The dependency is the package name in `Depends` or
    /// `Pre-Depends` (any alternative, version constraint, or qualifier) or this
    /// binary package's name. Extra is the command and `(does not satisfy ucf)`.
    /// The Control file and line are the Location. Applies to debs and udebs.
    MaintainerScriptNeedsDependsOnUcf,
    code = "LN0195",
    tag = "maintainer-script-needs-depends-on-ucf",
    default_selected = true,
    experimental = false,
}

static NEED: LazyLock<CommandNeed> = LazyLock::new(|| CommandNeed::new(r"\bucf\s", "ucf"));

impl BinaryPackageRule for MaintainerScriptNeedsDependsOnUcf {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        flag_missing_dependency(ctx, &NEED);
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(name: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file(name, TestFile::new(body))
    }

    #[test]
    fn test() {
        let pass = vec![
            script("postinst", "#!/bin/sh\nset -e\n"),
            script(
                "postinst",
                "#!/bin/sh\nucf /usr/share/foo/configuration /etc/foo.conf\n",
            )
            .field("Depends", "ucf"),
            script(
                "postinst",
                "#!/bin/sh\nwhich ucf >/dev/null || ucf src dest\n",
            ),
            script("postinst", "#!/usr/bin/perl\nuc\\\nf src dest\n"),
        ];
        let fail = vec![
            script(
                "postinst",
                "#!/bin/sh\nucf /usr/share/foo/configuration /etc/foo.conf\n",
            ),
            script("config", "#!/bin/sh\nucf src dest\n").field("Suggests", "ucf"),
        ];
        BinaryPackageTester::new(MaintainerScriptNeedsDependsOnUcf, pass, fail).test_and_snapshot();
    }
}
