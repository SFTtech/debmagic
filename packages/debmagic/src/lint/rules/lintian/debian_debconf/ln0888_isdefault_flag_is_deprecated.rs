use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, scripts};

pub struct IsdefaultFlagIsDeprecated;

declare_rule! {
    /// A maintainer script sets the deprecated `isdefault` flag.
    ///
    /// In `config`, `prerm`, `postrm`, `preinst`, or `postinst` (regular files
    /// that start with `#!`), a logical line matching `db_fset` followed by
    /// `isdefault` on the same line is reported once per script, at the line
    /// that ends the statement (after a trailing `\` continuation). Comments
    /// are dropped first, and the match does not cross a line break. The
    /// Extra and Location line are as lintian reports them, with an empty
    /// Extra. Udebs are checked.
    IsdefaultFlagIsDeprecated,
    code = "LN0888",
    tag = "isdefault-flag-is-deprecated",
    default_selected = true,
    experimental = false,
}

static DB_FSET_ISDEFAULT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"db_fset.*isdefault").expect("valid regex"));

impl BinaryPackageRule for IsdefaultFlagIsDeprecated {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let hits: Vec<(PathBuf, u32)> = {
            let Some(tree) = ctx.control_files() else {
                return;
            };
            scripts::scan(tree)
                .into_iter()
                .filter_map(|script| {
                    script
                        .lines
                        .iter()
                        .find(|line| DB_FSET_ISDEFAULT.is_match(&line.text))
                        .map(|line| (script.path, line.position))
                })
                .collect()
        };
        for (path, line) in hits {
            ctx.diagnostic(Diagnostic::warn("").with_location(files::located(path, Some(line))));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn postinst(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("postinst", TestFile::new(body))
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            postinst("#!/bin/sh\ndb_fset debconf/x seen true\n"),
            postinst("db_fset debconf/x isdefault true\n"),
            postinst("#!/bin/sh\n# db_fset debconf/x isdefault true\n"),
            postinst("#!/bin/sh\ndb_fset \\\n debconf/x isdefault true\n"),
        ];
        let fail = vec![
            postinst("#!/bin/sh\ndb_fset debconf/x isdefault true\n"),
            postinst("#!/bin/sh\ndb_fset debconf/x isdefault true\n").udeb(),
            postinst("#!/bin/sh\n\n\ndb_fset debconf/x isdefault false\ndb_fset y isdefault\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
                "config",
                TestFile::new("#!/bin/sh\ndb_fset debconf/x isdefault true\n"),
            ),
        ];
        BinaryPackageTester::new(IsdefaultFlagIsDeprecated, pass, fail).test_and_snapshot();
    }
}
