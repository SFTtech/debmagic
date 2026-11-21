use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::package_relation::PackageRelation;
use crate::lint::rule::BinaryPackageRule;

use super::{files, scripts};

pub struct PostrmDoesNotPurgeDebconf;

declare_rule! {
    /// A `postrm` does not call `db_purge`, and the package does not provide
    /// debconf itself.
    ///
    /// A `postrm` script (a regular file with `#!`) that never mentions
    /// `db_purge` outside comments is reported at the script, with an empty
    /// Extra. A `postrm` that is missing, a symlink, or not a regular file is
    /// reported with Extra `postrm` and no Location; a udeb is exempt only in
    /// that case, as in lintian. A `postrm` without `#!` is skipped. The
    /// package is exempt when its `Provides` plus its own name imply a debconf
    /// provider (`debconf`, `debconf-2.0`, `cdebconf`, `cdebconf-udeb`,
    /// `libdebconfclient0`, or `libdebconfclient0-udeb`).
    PostrmDoesNotPurgeDebconf,
    code = "LN0883",
    tag = "postrm-does-not-purge-debconf",
    default_selected = true,
    experimental = false,
}

/// Packages that provide debconf, OR'd, from the Check's `$ANY_DEBCONF`.
const ANY_DEBCONF: &str =
    "debconf | debconf-2.0 | cdebconf | cdebconf-udeb | libdebconfclient0 | libdebconfclient0-udeb";

enum Hit {
    Script(PathBuf),
    Missing,
}

impl BinaryPackageRule for PostrmDoesNotPurgeDebconf {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let udeb = ctx.is_udeb();
        let exempt = provides_debconf(ctx);
        let hit = {
            let Some(tree) = ctx.control_files() else {
                return;
            };
            match tree.get("postrm") {
                Some(entry) if is_regular(entry) => scripts::scan(tree)
                    .into_iter()
                    .find(|script| script.path.file_name().is_some_and(|name| name == "postrm"))
                    .filter(|script| {
                        !exempt
                            && !script
                                .lines
                                .iter()
                                .any(|line| line.text.contains("db_purge"))
                    })
                    .map(|script| Hit::Script(script.path)),
                _ => (!udeb && !exempt).then_some(Hit::Missing),
            }
        };
        match hit {
            Some(Hit::Script(path)) => {
                ctx.diagnostic(Diagnostic::warn("").with_location(files::located(path, None)));
            }
            Some(Hit::Missing) => ctx.diagnostic(Diagnostic::warn("postrm")),
            None => {}
        }
    }
}

fn is_regular(entry: &crate::lint::file_tree::Entry) -> bool {
    use crate::lint::file_tree::EntryKind;
    matches!(entry.kind(), EntryKind::RegularFile | EntryKind::Hardlink)
}

/// `Provides` plus the package's own name imply a debconf provider.
fn provides_debconf(ctx: &BinaryPackageContext<'_>) -> bool {
    let provides = ctx
        .installation_control()
        .fields()
        .relation("Provides")
        .unwrap_or_else(|| PackageRelation::parse(""));
    provides
        .and(&PackageRelation::parse(ctx.package_name()))
        .satisfies(ANY_DEBCONF)
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn postrm(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("postrm", TestFile::new(body))
    }

    #[test]
    fn binary() {
        let pass = vec![
            postrm("#!/bin/sh\nset -e\n. /usr/share/debconf/confmodule\ndb_purge\n"),
            postrm("#!/bin/sh\ndb_purge\n"),
            postrm("echo no shebang\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .field("Provides", "debconf-2.0"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).package("debconf"),
            postrm("#!/bin/sh\necho\n").field("Provides", "cdebconf"),
        ];
        let fail = vec![
            postrm("#!/bin/sh\nset -e\n"),
            postrm("#!/bin/sh\n# db_purge\n"),
            postrm("#!/bin/sh\necho\n").field("Depends", "debconf"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postrm", TestFile::symlink("missing")),
            postrm("#!/bin/sh\necho\n").udeb(),
        ];
        BinaryPackageTester::new(PostrmDoesNotPurgeDebconf, pass, fail).test_and_snapshot();
    }
}
