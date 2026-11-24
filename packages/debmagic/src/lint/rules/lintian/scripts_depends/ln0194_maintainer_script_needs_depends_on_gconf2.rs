use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::depends::{CommandNeed, flag_missing_dependency};

pub struct MaintainerScriptNeedsDependsOnGconf2;

declare_rule! {
    /// A maintainer script calls `gconf-schemas` without a dependency on `gconf2`.
    ///
    /// The call is that command followed by whitespace, outside `postrm`. A line
    /// that tests `-x`, runs `which` or `type`, runs `command`, or contains
    /// `|| true` is exempt. The dependency is the package name in `Depends` or
    /// `Pre-Depends` (any alternative, version constraint, or qualifier) or this
    /// binary package's name. Extra is the command and `(does not satisfy gconf2)`.
    /// The Control file and line are the Location. Applies to debs and udebs.
    MaintainerScriptNeedsDependsOnGconf2,
    code = "LN0194",
    tag = "maintainer-script-needs-depends-on-gconf2",
    default_selected = true,
    experimental = false,
}

static NEED: LazyLock<CommandNeed> =
    LazyLock::new(|| CommandNeed::new(r"\bgconf-schemas\s", "gconf2"));

impl BinaryPackageRule for MaintainerScriptNeedsDependsOnGconf2 {
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
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            script("postinst", "#!/bin/sh\nset -e\n"),
            script("postinst", "#!/bin/sh\ngconf-schemas --register foo\n")
                .field("Depends", "gconf2"),
            script("postinst", "#!/bin/sh\ngconf-schemas --register foo\n")
                .field("Pre-Depends", "gconf2"),
            script("postinst", "#!/bin/sh\ngconf-schemas --register foo\n")
                .field("Depends", "gconf2 (>= 2.28.1-2)"),
            script("postinst", "#!/bin/sh\ngconf-schemas --register foo\n")
                .field("Depends", "foo | gconf2"),
            script("postinst", "#!/bin/sh\ngconf-schemas --register foo\n")
                .field("Depends", "gconf2:any"),
            script("postinst", "#!/bin/sh\ngconf-schemas --register foo\n").package("gconf2"),
            script("postrm", "#!/bin/sh\ngconf-schemas --register foo\n"),
            script(
                "postinst",
                "#!/bin/sh\nif [ -x /usr/bin/gconf-schemas ]; then gconf-schemas --register foo; fi\n",
            ),
            script("postinst", "#!/bin/sh\nwhich gconf-schemas >/dev/null\n"),
            script("postinst", "#!/bin/sh\ntype gconf-schemas >/dev/null\n"),
            script(
                "postinst",
                "#!/bin/sh\ncommand -v gconf-schemas >/dev/null\n",
            ),
            script(
                "postinst",
                "#!/bin/sh\ngconf-schemas --register foo || true\n",
            ),
            script(
                "postinst",
                "#!/bin/sh\ngconf-schemas --register foo||true\n",
            ),
            script("postinst", "#!/bin/sh\n# gconf-schemas --register foo\n"),
            script(
                "postinst",
                "#!/bin/sh\ntrue # gconf-schemas --register foo\n",
            ),
            script("postinst", "gconf-schemas --register foo\n"),
            script("postinst", "#!/bin/sh\ngconf-schemas \\\n"),
            script(
                "postinst",
                "#!/usr/bin/perl\ngconf-\\\nschemas --register foo\n",
            ),
            script("triggers", "#!/bin/sh\ngconf-schemas --register foo\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::symlink("foo")),
            script("postinst", "#!/bin/sh\necho ok\n"),
        ];
        let fail = vec![
            script("postinst", "#!/bin/sh\ngconf-schemas --register foo\n"),
            script("postinst", "#!/bin/sh\ngconf-schemas a\ngconf-schemas b\n"),
            script(
                "postinst",
                "#!/bin/sh\ngconf-schemas --register foo || exit 0\n",
            ),
            script(
                "postinst",
                "#!/bin/sh\ngconf-schemas \\\n# still continued\n\n  --register foo\n",
            ),
            script("postinst", "#!/bin/sh\ngconf-schemas --register foo\n")
                .field("Recommends", "gconf2"),
            script("postinst", "#!/bin/sh\ngconf-schemas --register foo\n")
                .field("Depends", "gconf2-bin"),
            script("postinst", "#!/bin/sh\ngconf-schemas --register foo\n").udeb(),
            script("my-postinst", "#!/bin/sh\ngconf-schemas --register foo\n"),
            script(
                "postinst",
                "#!/usr/bin/perl\ngconf-schemas --register foo\n",
            ),
            script(
                "postinst",
                "#!/usr/bin/env sh\ngconf-schemas --register foo\n",
            ),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file(
                    "postinst",
                    TestFile::new("#!/bin/sh\ngconf-schemas --register a\n"),
                )
                .control_file(
                    "postrm",
                    TestFile::new("#!/bin/sh\ngconf-schemas --register a\n"),
                ),
        ];
        BinaryPackageTester::new(MaintainerScriptNeedsDependsOnGconf2, pass, fail)
            .test_and_snapshot();
    }
}
