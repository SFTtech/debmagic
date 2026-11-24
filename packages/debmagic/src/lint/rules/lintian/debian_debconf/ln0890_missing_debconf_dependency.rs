use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::package_relation::PackageRelation;
use crate::lint::rule::BinaryPackageRule;

use super::{files, satisfy};

pub struct MissingDebconfDependency;

declare_rule! {
    /// A package with debconf control files does not depend on a debconf
    /// provider.
    ///
    /// The Check runs when `config` or `templates` is a regular file, or when
    /// `preinst` loads a confmodule. When `preinst` loads one, the Rule
    /// defers to `missing-debconf-dependency-for-preinst`. Otherwise the
    /// package must imply `debconf`, `debconf-2.0`, `cdebconf`,
    /// `cdebconf-udeb`, `libdebconfclient0`, or `libdebconfclient0-udeb`
    /// through `Pre-Depends`, `Depends`, `Provides`, or its own name, or
    /// imply `dbconfig-common` through the same fields. `Recommends` and
    /// `Suggests` do not count, and an architecture restriction or `:any`
    /// qualifier does not imply the unrestricted name. The Extra and Location
    /// are empty. Udebs are checked too.
    MissingDebconfDependency,
    code = "LN0890",
    tag = "missing-debconf-dependency",
    default_selected = true,
    experimental = false,
}

/// Packages that provide debconf, OR'd, from the Check's `$ANY_DEBCONF`.
const ANY_DEBCONF: &str =
    "debconf | debconf-2.0 | cdebconf | cdebconf-udeb | libdebconfclient0 | libdebconfclient0-udeb";

impl BinaryPackageRule for MissingDebconfDependency {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let (gate_open, uses_preinst) = {
            let Some(tree) = ctx.control_files() else {
                return;
            };
            let gate = files::gate(tree);
            (gate.open(), gate.preinst_debconf)
        };
        if !gate_open || uses_preinst {
            return;
        }
        if strong_provides_or_self(ctx).satisfies(ANY_DEBCONF) || satisfy::uses_dbconfig(ctx) {
            return;
        }
        ctx.diagnostic(Diagnostic::warn(""));
    }
}

/// Lintian's `alldependencies`: `Pre-Depends`, `Depends`, `Provides`, and
/// the package itself, AND-ed.
fn strong_provides_or_self(ctx: &BinaryPackageContext<'_>) -> PackageRelation {
    ctx.installation_control()
        .fields()
        .relations_and(&["Pre-Depends", "Depends", "Provides"])
        .and(&PackageRelation::parse(ctx.package_name()))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn config(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("config", TestFile::new(body))
    }

    #[test]
    fn binary() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postrm", TestFile::new("#!/bin/sh\n")),
            config("#!/bin/sh\n").field("Pre-Depends", "debconf"),
            config("#!/bin/sh\n").field("Depends", "debconf (>= 1.5)"),
            config("#!/bin/sh\n").field("Depends", "cdebconf | debconf-2.0"),
            config("#!/bin/sh\n").field("Provides", "debconf-2.0"),
            config("#!/bin/sh\n").package("debconf"),
            config("#!/bin/sh\n").field("Depends", "dbconfig-common"),
            config("#!/bin/sh\n").field("Pre-Depends", "debconf").udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()).control_file(
                "preinst",
                TestFile::new("#!/bin/sh\n. /usr/share/debconf/confmodule\n"),
            ),
        ];
        let fail = vec![
            config("#!/bin/sh\n"),
            config("#!/bin/sh\n").field("Recommends", "debconf"),
            config("#!/bin/sh\n").field("Depends", "debconf [amd64]"),
            config("#!/bin/sh\n").field("Depends", "debconf:any"),
            config("#!/bin/sh\n").field("Depends", "debconf | mail-transport-agent"),
            config("#!/bin/sh\n").udeb(),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("templates", TestFile::new("Template: a/b\nType: note\n")),
        ];
        BinaryPackageTester::new(MissingDebconfDependency, pass, fail).test_and_snapshot();
    }
}
