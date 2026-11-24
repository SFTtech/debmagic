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
            // The Check returns before every installable Tag unless the package
            // has `config`, `templates`, or a debconf-using `preinst`.
            if !files::gate(tree).open() {
                return;
            }
            match tree.get("postrm") {
                Some(entry) if is_regular(entry) => {
                    // The Check keeps one `db_purge` flag across all scripts, so a
                    // `db_purge` in `config` or `prerm` (scanned before `postrm`)
                    // also satisfies this Tag.
                    let mut db_purge = false;
                    let mut hit = None;
                    for script in scripts::scan(tree) {
                        db_purge |= script
                            .lines
                            .iter()
                            .any(|line| line.text.contains("db_purge"));
                        if script.path.file_name().is_some_and(|name| name == "postrm")
                            && !exempt
                            && !db_purge
                        {
                            hit = Some(Hit::Script(script.path));
                        }
                    }
                    hit
                }
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

    /// A package the Check examines: it ships a `config` file.
    fn debconf_package() -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("config", TestFile::new("#!/bin/sh\n"))
    }

    fn postrm(body: &str) -> BinaryPackageCase {
        debconf_package().control_file("postrm", TestFile::new(body))
    }

    #[test]
    fn binary() {
        let pass = vec![
            postrm("#!/bin/sh\nset -e\n. /usr/share/debconf/confmodule\ndb_purge\n"),
            postrm("#!/bin/sh\ndb_purge\n"),
            postrm("echo no shebang\n"),
            debconf_package().udeb(),
            debconf_package().field("Provides", "debconf-2.0"),
            debconf_package().package("debconf"),
            postrm("#!/bin/sh\necho\n").field("Provides", "cdebconf"),
            // The Check's `db_purge` flag is shared across scripts, so one in
            // `config` or `prerm` satisfies the postrm check.
            debconf_package()
                .control_file("config", TestFile::new("#!/bin/sh\ndb_purge\n"))
                .control_file("postrm", TestFile::new("#!/bin/sh\necho\n")),
            debconf_package()
                .control_file("prerm", TestFile::new("#!/bin/sh\ndb_purge\n"))
                .control_file("postrm", TestFile::new("#!/bin/sh\necho\n")),
            // Not a debconf package: lintian returns before the maintainer
            // scripts, so neither a script nor a missing postrm is reported.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postrm", TestFile::new("#!/bin/sh\necho\n")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
        ];
        let fail = vec![
            postrm("#!/bin/sh\nset -e\n"),
            postrm("#!/bin/sh\n# db_purge\n"),
            postrm("#!/bin/sh\necho\n").field("Depends", "debconf"),
            debconf_package(),
            debconf_package().control_file("postrm", TestFile::symlink("missing")),
            postrm("#!/bin/sh\necho\n").udeb(),
            // `templates` alone opens the gate.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("templates", TestFile::new("Template: a/b\nType: note\n")),
            // A `preinst` that loads the confmodule opens the gate.
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file(
                    "preinst",
                    TestFile::new("#!/bin/sh\n. /usr/share/debconf/confmodule\n"),
                )
                .control_file("postrm", TestFile::new("#!/bin/sh\necho\n")),
        ];
        BinaryPackageTester::new(PostrmDoesNotPurgeDebconf, pass, fail).test_and_snapshot();
    }

    /// The Check returns before its installable Tags unless the package ships
    /// `config`, `templates`, or a `preinst` loading the debconf confmodule, so
    /// a package with none of them is not examined at all.
    #[test]
    fn gate_closed_without_debconf_control_files_reports_nothing() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postrm", TestFile::new("#!/bin/sh\necho\n")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("preinst", TestFile::new("#!/bin/sh\necho\n")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
        ];
        BinaryPackageTester::new(
            PostrmDoesNotPurgeDebconf,
            pass,
            Vec::<BinaryPackageCase>::new(),
        )
        .test();
    }

    /// Either of the gate's openers alone makes the Check examine the package:
    /// a `templates` file, or a `preinst` that loads the confmodule.
    #[test]
    fn gate_opens_on_templates_or_preinst_confmodule() {
        let fail = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("templates", TestFile::new("Template: a/b\nType: note\n")),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
                "preinst",
                TestFile::new("#!/bin/sh\n. /usr/share/debconf/confmodule\n"),
            ),
        ];
        BinaryPackageTester::new(
            PostrmDoesNotPurgeDebconf,
            Vec::<BinaryPackageCase>::new(),
            fail,
        )
        .test();
    }

    /// The Check keeps one `db_purge` flag across the scripts it scans in order
    /// (`config`, `prerm`, `postrm`), so a `db_purge` in `config` satisfies the
    /// `postrm` Tag. A commented `db_purge` is not a call.
    #[test]
    fn db_purge_in_config_satisfies_postrm() {
        let pass = vec![
            debconf_package()
                .control_file("config", TestFile::new("#!/bin/sh\ndb_purge\n"))
                .control_file("postrm", TestFile::new("#!/bin/sh\necho\n")),
        ];
        // The comment form is not a `db_purge` call, in any script.
        let fail = vec![
            debconf_package()
                .control_file("config", TestFile::new("#!/bin/sh\n# db_purge\n"))
                .control_file("postrm", TestFile::new("#!/bin/sh\necho\n")),
        ];
        BinaryPackageTester::new(PostrmDoesNotPurgeDebconf, pass, fail).test();
    }
}
