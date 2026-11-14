use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::depends::{CommandNeed, flag_missing_dependency};

pub struct MaintainerScriptNeedsDependsOnXfontsUtils;

declare_rule! {
    /// A maintainer script calls `update-fonts-alias`, `update-fonts-dir`, or
    /// `update-fonts-scale` without a dependency on `xfonts-utils`.
    ///
    /// The call is one of those commands followed by whitespace, outside `postrm`.
    /// A line that tests `-x`, runs `which` or `type`, runs `command`, or contains
    /// `|| true` is exempt. The dependency is the package name in `Depends` or
    /// `Pre-Depends` (any alternative, version constraint, or qualifier) or this
    /// binary package's name. Extra is the command and `(does not satisfy xfonts-utils)`.
    /// The Control file and line are the Location. Applies to debs and udebs.
    MaintainerScriptNeedsDependsOnXfontsUtils,
    code = "LN0197",
    tag = "maintainer-script-needs-depends-on-xfonts-utils",
    default_selected = true,
    experimental = false,
}

static NEED: LazyLock<CommandNeed> =
    LazyLock::new(|| CommandNeed::new(r"\bupdate-fonts-(?:alias|dir|scale)\s", "xfonts-utils"));

impl BinaryPackageRule for MaintainerScriptNeedsDependsOnXfontsUtils {
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
                "#!/bin/sh\nupdate-fonts-alias blah\nupdate-fonts-dir blah\nupdate-fonts-scale blah\n",
            )
            .field("Depends", "xfonts-utils"),
            script("postinst", "#!/bin/sh\nupdate-fonts-foo blah\n"),
            script("postinst", "#!/bin/sh\nupdate-fonts-dir\n"),
        ];
        let fail = vec![
            script("postinst", "#!/bin/sh\nupdate-fonts-scale blah\n"),
            script(
                "postinst",
                "#!/bin/sh\nif false ; then\n    update-fonts-alias blah\n    update-fonts-dir blah\n    update-fonts-scale blah\nfi\n",
            ),
            script(
                "postinst",
                "#!/bin/sh\nupdate-fonts-alias a; update-fonts-dir b\n",
            ),
        ];
        BinaryPackageTester::new(MaintainerScriptNeedsDependsOnXfontsUtils, pass, fail)
            .test_and_snapshot();
    }
}
