use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MaintainerScriptRemovesDeviceFiles;

declare_rule! {
    /// A shell maintainer script removes a device node.
    ///
    /// A logical line that starts with `rm` and names a path under `/dev/`,
    /// other than `/dev/shm/…` and dotfiles. Arguments before the device are
    /// included; a redirection (`rm /tmp/foo > /dev/null`) is not. The extra
    /// is the match in double quotes. Location is the Control file and the
    /// physical line. Applies to udebs.
    MaintainerScriptRemovesDeviceFiles,
    code = "LN0162",
    tag = "maintainer-script-removes-device-files",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MaintainerScriptRemovesDeviceFiles {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        for script in scan::maintainer_scripts(files) {
            if !script.is_shell {
                continue;
            }
            for line in script.lines {
                let Some(matched) = scan::removes_device(&line.text) else {
                    continue;
                };
                ctx.diagnostic(Diagnostic::error(format!("\"{matched}\"")).with_location(
                    Location {
                        path: script.path.clone(),
                        line: Some(line.number),
                        column: None,
                    },
                ));
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
            with_script("postinst", "#!/bin/sh\n# rm /dev/null\n"),
            with_script("postinst", "#!/bin/sh\nset -e\nrm /dev/shm/test\n"),
            with_script("postinst", "#!/bin/sh\nset -e\nrm /dev/.hiddenfile\n"),
            with_script("postinst", "#!/bin/sh\nset -e\nrm /tmp/foo > /dev/null\n"),
            with_script("postinst", "#!/bin/sh\nset -e\necho rm /dev/null\n"),
            with_script("postinst", "#!/usr/bin/perl\nrm /dev/null\n"),
            with_script("triggers", "#!/bin/sh\nrm /dev/null\n"),
            BinaryPackageCase::from(vec![(
                "usr/bin/example",
                TestFile::new("#!/bin/sh\nrm /dev/null\n"),
            )]),
        ];
        let fail = vec![
            with_script("postinst", "#!/bin/sh\nset -e\nrm /dev/null\n"),
            with_script("postrm", "#!/bin/bash\nrm /tmp/foo /dev/device\n"),
            with_script("postrm", "#!/bin/bash\nrm\t/dev/device1\n"),
            with_script("postinst", "#!/bin/sh\nset -e\nrm -f /dev/null\n"),
            with_script("postinst", "#!/bin/sh\nset -e\nrm /dev/shm/foo /dev/null\n"),
            with_script("postinst", "#!/bin/sh\nrm /dev/null\n").udeb(),
        ];
        BinaryPackageTester::new(MaintainerScriptRemovesDeviceFiles, pass, fail)
            .test_and_snapshot();
    }
}
