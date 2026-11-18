use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::init_script;

pub struct InitDScriptMissingLsbKeyword;

declare_rule! {
    /// An `etc/init.d` script has an LSB section, but the first section does
    /// not set a required keyword.
    ///
    /// Required keywords are `Provides`, `Required-Start`, `Required-Stop`,
    /// `Default-Start`, and `Default-Stop`. A keyword counts as set when its
    /// line is present, even with an empty value. Each missing keyword is its
    /// own finding. Extra is the lowercased keyword. The script path is the
    /// Location. A script without a BEGIN marker is not checked. The same
    /// scripts are skipped as for the required-option Tag.
    InitDScriptMissingLsbKeyword,
    code = "LN0746",
    tag = "init.d-script-missing-lsb-keyword",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for InitDScriptMissingLsbKeyword {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        for script in scripts(ctx) {
            if !script.section.begun {
                continue;
            }
            for keyword in init_script::LSB_REQUIRED_KEYWORDS {
                let set = script
                    .section
                    .keywords
                    .iter()
                    .any(|(_, name)| name == keyword);
                if !set {
                    ctx.diagnostic(Diagnostic::warn(keyword).with_location(at(&script.path)));
                }
            }
        }
    }
}

fn scripts(ctx: &mut BinaryPackageContext<'_>) -> Vec<init_script::InitDScript> {
    let package = ctx.package_name().to_string();
    let registered = {
        let Some(control) = ctx.control_files() else {
            return Vec::new();
        };
        init_script::registered_names(control)
    };
    let Some(data) = ctx.data_files() else {
        return Vec::new();
    };
    init_script::init_d_scripts(data, &registered, &package)
}

fn at(path: &Path) -> Location {
    Location {
        path: path.to_path_buf(),
        line: None,
        column: None,
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(name: &str, body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(format!("etc/init.d/{name}"), TestFile::new(body))])
    }

    const FULL: &str = "\
#!/bin/sh
### BEGIN INIT INFO
# Provides: a
# Required-Start:
# Required-Stop: $local_fs
# Default-Start: 2 3 4 5
# Default-Stop: 0 1 6
### END INIT INFO
exit 0
";

    #[test]
    fn test() {
        let pass = vec![
            script("ok", FULL),
            script("no-lsb", "#!/bin/sh\nexit 0\n"),
            script(
                "skeleton",
                "#!/bin/sh\n### BEGIN INIT INFO\n### END INIT INFO\n",
            ),
            script("outside", "#!/bin/sh\n# Provides: a\n"),
            script("pkg", "#!/bin/sh\n### BEGIN INIT INFO\n### END INIT INFO\n")
                .package("sysvinit"),
        ];
        let fail = vec![
            script(
                "two",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n# Required-Stop: b\n# Default-Start: 2\n### END INIT INFO\n",
            ),
            script(
                "empty",
                "#!/bin/sh\n### BEGIN INIT INFO\n### END INIT INFO\n",
            ),
            script(
                "unterminated",
                "#!/bin/sh\n### BEGIN INIT INFO\n# Provides: a\n\n# Required-Start: b\n",
            ),
            script(
                "udeb-script",
                "#!/bin/sh\n### BEGIN INIT INFO\n### END INIT INFO\n",
            )
            .udeb(),
        ];
        BinaryPackageTester::new(InitDScriptMissingLsbKeyword, pass, fail).test_and_snapshot();
    }
}
