use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct MaintscriptCallsLdconfig;

declare_rule! {
    /// A maintainer script calls `ldconfig`.
    ///
    /// One hit per Control file when a line contains `ldconfig` before any
    /// `#` on that line. No shebang is required, and the call is not joined
    /// across lines. Extra is empty. The Location is the file with no line
    /// number. A udeb whose Control file is exactly `postinst` is skipped
    /// (lintian reports that case as another Tag).
    MaintscriptCallsLdconfig,
    code = "LN0188",
    tag = "maintscript-calls-ldconfig",
    default_selected = true,
    experimental = false,
}

static LDCONFIG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^[^#]*\bldconfig\b").expect("valid regex"));

impl BinaryPackageRule for MaintscriptCallsLdconfig {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let udeb = ctx.is_udeb();
        let Some(files) = ctx.control_files() else {
            return;
        };
        let hits: Vec<_> = scan::read_scripts(files)
            .into_iter()
            .filter(|(path, _)| !(udeb && path == Path::new("postinst")))
            .filter(|(_, bytes)| {
                let text = String::from_utf8_lossy(bytes);
                LDCONFIG.is_match(&text)
            })
            .map(|(path, _)| path)
            .collect();
        for path in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(Location {
                path,
                line: None,
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
            control(
                "postinst",
                "#!/bin/sh\nset -e\n\n# No ldconfig\nif false ; then true ; fi\n",
            ),
            control("postinst", "#!/bin/sh\n# ldconfig\n"),
            control("postinst", "#!/bin/sh\necho foo # ldconfig\n"),
            control("postinst", "#!/bin/sh\nnotldconfig\n"),
            control("postinst", "#!/bin/sh\nld\\\nconfig\n"),
            control("triggers", "#!/bin/sh\nldconfig\n"),
            control("postinst", "#!/bin/sh\nldconfig\n").udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::symlink("foo")),
        ];
        let fail = vec![
            control("postinst", "#!/bin/sh\nset -e\n\nldconfig\n\n#DEBHELPER#\n"),
            control("postrm", "#!/bin/bash\n       ldconfig\n"),
            control("prerm", "#!/bin/sh\nldconfig\n"),
            control("preinst", "#!/bin/sh\n[ \"remove\" =\"$1\" ] && ldconfig\n"),
            control("postrm", "ldconfig\n"),
            control("postinst", "#!/bin/sh\nfoo; ldconfig\nldconfig\n"),
            control("postinst", "#!/bin/sh\nldconfig.real\n"),
            control("my-postinst", "#!/bin/sh\nldconfig\n").udeb(),
            control("prerm", "#!/bin/sh\nldconfig\n").udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::new("#!/bin/sh\nldconfig\n"))
                .control_file("postrm", TestFile::new("#!/bin/sh\nldconfig\n"))
                .udeb(),
        ];
        BinaryPackageTester::new(MaintscriptCallsLdconfig, pass, fail).test_and_snapshot();
    }
}
