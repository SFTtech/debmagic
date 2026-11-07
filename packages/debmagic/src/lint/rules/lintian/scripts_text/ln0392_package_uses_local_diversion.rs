use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct PackageUsesLocalDiversion;

declare_rule! {
    /// A maintainer script calls `dpkg-divert --local`.
    ///
    /// Any interpreter. `--help`, `--list`, `--truename`, and `--version`
    /// are not diversions. Extra is empty. The Location is the last physical
    /// line of the logical line.
    PackageUsesLocalDiversion,
    code = "LN0392",
    tag = "package-uses-local-diversion",
    default_selected = true,
    experimental = false,
}

static DIVERT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        "{}(?:/usr/sbin/)?dpkg-divert\\s",
        scan::LEADING_PATTERN
    ))
    .expect("valid regex")
});
static NOT_A_DIVERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"--(?:help|list|truename|version)").expect("valid regex"));

impl BinaryPackageRule for PackageUsesLocalDiversion {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.control_files() else {
            return;
        };
        let hits: Vec<_> = scan::script_lines(files)
            .into_iter()
            .filter(|line| DIVERT.is_match(&line.text))
            .filter(|line| !NOT_A_DIVERSION.is_match(&line.text))
            .filter(|line| line.text.contains("--local"))
            .map(|line| (line.path, line.line))
            .collect();
        for (path, line) in hits {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: Some(line),
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
                "preinst",
                "#!/bin/sh\ndpkg-divert --rename --package foo /bin/bar\n",
            ),
            control("preinst", "#!/bin/sh\ndpkg-divert --help --local\n"),
            control("preinst", "#!/bin/sh\n# dpkg-divert --local /bin/bar\n"),
            control("preinst", "dpkg-divert --local /bin/bar\n"),
        ];
        let fail = vec![
            control(
                "preinst",
                "#!/bin/sh\ndpkg-divert --local --rename /bin/bar\n",
            ),
            control(
                "postrm",
                "#!/bin/sh\n/usr/sbin/dpkg-divert --local /bin/bar\n",
            ),
        ];
        BinaryPackageTester::new(PackageUsesLocalDiversion, pass, fail).test_and_snapshot();
    }
}
