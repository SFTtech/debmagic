use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, satisfy};

pub struct MissingDebconfDependencyForPreinst;

declare_rule! {
    /// `preinst` loads debconf but `Pre-Depends` does not require it.
    ///
    /// A confmodule path or `Debconf::Client::ConfModule` in `preinst` counts,
    /// comments aside, with or without a shebang. `Pre-Depends` must imply
    /// `debconf`, `debconf-2.0`, `cdebconf`, `cdebconf-udeb`,
    /// `libdebconfclient0`, or `libdebconfclient0-udeb`. `Depends` and the
    /// package's own name do not. An architecture restriction or `:any`
    /// qualifier does not imply the unrestricted name. Extra and Location are
    /// empty. Udebs are not checked.
    MissingDebconfDependencyForPreinst,
    code = "LN0246",
    tag = "missing-debconf-dependency-for-preinst",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MissingDebconfDependencyForPreinst {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let uses_preinst = {
            let Some(files) = ctx.control_files() else {
                return;
            };
            files::gate(files).preinst_debconf
        };
        if !uses_preinst || satisfy::pre_depends_on_debconf(ctx) {
            return;
        }
        ctx.diagnostic(Diagnostic::warn(""));
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn preinst(body: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("preinst", TestFile::new(body))
    }

    #[test]
    fn test() {
        let debconf = "#!/bin/sh\n. /usr/share/debconf/confmodule\n";
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            preinst("#!/bin/sh\ntrue\n"),
            preinst("# . /usr/share/debconf/confmodule\n"),
            preinst(debconf).field("Pre-Depends", "debconf"),
            preinst(debconf).field("Pre-Depends", "debconf | cdebconf"),
            preinst(debconf).field("Pre-Depends", "debconf (>= 1.5.0)"),
            preinst(debconf).field("Pre-Depends", "libdebconfclient0"),
            preinst(debconf).field("Pre-Depends", "cdebconf-udeb"),
            preinst(debconf).udeb(),
            preinst("#!/usr/bin/perl\nuse Debconf::Client::ConfModule;\n")
                .field("Pre-Depends", "debconf-2.0"),
        ];
        let fail = vec![
            preinst(debconf),
            preinst(debconf).field("Depends", "debconf"),
            preinst(debconf).field("Pre-Depends", "debconf | mail-transport-agent"),
            preinst(debconf).field("Pre-Depends", "debconf [amd64]"),
            preinst(debconf).field("Pre-Depends", "debconf:any"),
            preinst(". /usr/share/debconf/confmodule.sh\n"),
            preinst(debconf).package("debconf"),
        ];
        BinaryPackageTester::new(MissingDebconfDependencyForPreinst, pass, fail)
            .test_and_snapshot();
    }
}
