use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, scripts};

pub struct LoadsObsoleteConfmodule;

declare_rule! {
    /// A maintainer script loads an obsolete debconf confmodule.
    ///
    /// The obsolete names are `/usr/share/debconf/confmodule.sh` and
    /// `Debian::DebConf::Client::ConfModule`. One hit per script, the first
    /// line. Extra is the matched name. The Location is the Check's script
    /// position. A file without `#!` is skipped. Udebs are not checked.
    LoadsObsoleteConfmodule,
    code = "LN0241",
    tag = "loads-obsolete-confmodule",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for LoadsObsoleteConfmodule {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let hits = {
            let Some(files) = ctx.control_files() else {
                return;
            };
            if !files::gate(files).open() {
                return;
            }
            let mut hits = Vec::new();
            for script in scripts::scan(files) {
                for line in &script.lines {
                    if let Some(module) = scripts::obsolete_module(&line.text) {
                        hits.push((script.path.clone(), line.position, module));
                        break;
                    }
                }
            }
            hits
        };
        for (path, line, module) in hits {
            ctx.diagnostic(
                Diagnostic::warn(module).with_location(files::located(path, Some(line))),
            );
        }
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
            script("config", "#!/bin/sh\n. /usr/share/debconf/confmodule\n"),
            script(
                "config",
                "#!/usr/bin/perl\nuse Debconf::Client::ConfModule;\n",
            ),
            script(
                "config",
                "#!/bin/sh\n# . /usr/share/debconf/confmodule.sh\n",
            ),
            script("config", ". /usr/share/debconf/confmodule.sh\n"),
            script("config", "#!/bin/sh\n. confmodule.sh\n"),
            script("postrm", "#!/bin/sh\n. /usr/share/debconf/confmodule.sh\n"),
            script("config", "#!/bin/sh\n. /usr/share/debconf/confmodule.sh\n").udeb(),
        ];
        let fail = vec![
            script(
                "preinst",
                "#!/bin/sh\nset -e\n\n# Obsolete name for the confmodule\n. /usr/share/debconf/confmodule.sh\n. /usr/share/debconf/confmodule\n",
            ),
            script(
                "config",
                "#!/usr/bin/perl\nuse Debian::DebConf::Client::ConfModule;\n",
            ),
            script(
                "config",
                "#!/bin/sh\n. /usr/share/debconf/confmodule.sh\n. /usr/share/debconf/confmodule.sh\n",
            ),
            script("preinst", "#!/bin/sh\n. /usr/share/debconf/confmodule.sh\n").control_file(
                "postrm",
                TestFile::new("#!/bin/sh\n. /usr/share/debconf/confmodule.sh\n"),
            ),
        ];
        BinaryPackageTester::new(LoadsObsoleteConfmodule, pass, fail).test_and_snapshot();
    }
}
