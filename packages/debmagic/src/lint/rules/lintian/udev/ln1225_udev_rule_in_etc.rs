use std::path::Path;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

pub struct UdevRuleInEtc;

declare_rule! {
    /// The Binary package installs a file under `etc/udev/rules.d/`, which is
    /// reserved for rules the administrator adds. System rules belong in
    /// `usr/lib/udev/rules.d/`.
    ///
    /// Every Data files entry (files, directories, links) whose path starts with
    /// `etc/udev/rules.d/` and has a non-space character after that prefix is a hit.
    /// The directory itself is not. Extra is empty; the path is the Location.
    UdevRuleInEtc,
    code = "LN1225",
    tag = "udev-rule-in-etc",
    default_selected = true,
    experimental = false,
}

const RULES_DIR: &str = "etc/udev/rules.d/";

impl BinaryPackageRule for UdevRuleInEtc {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let paths: Vec<_> = files
            .entries()
            .iter()
            .map(|entry| entry.path())
            .filter(|path| is_rule_in_etc(path))
            .map(Path::to_path_buf)
            .collect();
        for path in paths {
            ctx.diagnostic(Diagnostic::error("").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn is_rule_in_etc(path: &Path) -> bool {
    let Some(rest) = path.to_str().and_then(|p| p.strip_prefix(RULES_DIR)) else {
        return false;
    };
    rest.chars().next().is_some_and(|c| !c.is_whitespace())
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(vec![(
                "usr/lib/udev/rules.d/70-foo.rules",
                TestFile::new("x"),
            )]),
            BinaryPackageCase::from(vec![("etc/udev/rules.d", TestFile::directory())]),
            BinaryPackageCase::from(vec![("etc/udev/other/70-foo.rules", TestFile::new("x"))]),
        ];
        let fail = vec![
            BinaryPackageCase::from(vec![("etc/udev/rules.d/70-foo.rules", TestFile::new("x"))]),
            BinaryPackageCase::from(vec![("etc/udev/rules.d/sub", TestFile::directory())]),
            BinaryPackageCase::from(vec![(
                "etc/udev/rules.d/70-foo.rules",
                TestFile::symlink("/usr/lib/udev/rules.d/70-foo.rules"),
            )]),
            // A udeb is still a Binary package.
            BinaryPackageCase::from(vec![("etc/udev/rules.d/70-foo.rules", TestFile::new("x"))])
                .udeb(),
        ];
        BinaryPackageTester::new(UdevRuleInEtc, pass, fail).test_and_snapshot();
    }
}
