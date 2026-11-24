use std::sync::LazyLock;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::rule::BinaryPackageRule;

use super::depends::{CommandNeed, flag_missing_dependency};

pub struct MaintainerScriptNeedsDependsOnXmlCore;

declare_rule! {
    /// A maintainer script calls `update-xmlcatalog` without a dependency on `xml-core`.
    ///
    /// The call is that command followed by whitespace, outside `postrm`. A line
    /// that tests `-x`, runs `which` or `type`, runs `command`, or contains
    /// `|| true` is exempt. The dependency is the package name in `Depends` or
    /// `Pre-Depends` (any alternative, version constraint, or qualifier) or this
    /// binary package's name. Extra is the command and `(does not satisfy xml-core)`.
    /// The Control file and line are the Location. Applies to debs and udebs.
    MaintainerScriptNeedsDependsOnXmlCore,
    code = "LN0196",
    tag = "maintainer-script-needs-depends-on-xml-core",
    default_selected = true,
    experimental = false,
}

static NEED: LazyLock<CommandNeed> =
    LazyLock::new(|| CommandNeed::new(r"\bupdate-xmlcatalog\s", "xml-core"));

impl BinaryPackageRule for MaintainerScriptNeedsDependsOnXmlCore {
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
            script("postinst", "#!/bin/sh\nupdate-xmlcatalog --add\n").field("Depends", "xml-core"),
            script("prerm", "#!/bin/sh\nupdate-catalog --add\n"),
        ];
        let fail = vec![
            script("postinst", "#!/bin/sh\nupdate-xmlcatalog --add\n"),
            script(
                "postinst",
                "#!/bin/sh\nupdate-xmlcatalog --add \\\n    --root\n",
            ),
        ];
        BinaryPackageTester::new(MaintainerScriptNeedsDependsOnXmlCore, pass, fail)
            .test_and_snapshot();
    }
}
