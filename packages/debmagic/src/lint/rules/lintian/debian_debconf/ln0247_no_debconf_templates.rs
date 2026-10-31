use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::{files, satisfy};

pub struct NoDebconfTemplates;

declare_rule! {
    /// The package ships a `config` script and no `templates` file.
    ///
    /// A symlink does not count as either file. Depending on, pre-depending
    /// on, providing, or being `dbconfig-common` suppresses the Tag, because
    /// that package supplies the templates. `Recommends` does not, and an OR
    /// with another package does not. Extra and Location are empty. Udebs are
    /// not checked.
    NoDebconfTemplates,
    code = "LN0247",
    tag = "no-debconf-templates",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for NoDebconfTemplates {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        if ctx.is_udeb() {
            return;
        }
        let gate = {
            let Some(files) = ctx.control_files() else {
                return;
            };
            files::gate(files)
        };
        if gate.config && !gate.templates && !satisfy::uses_dbconfig(ctx) {
            ctx.diagnostic(Diagnostic::warn(""));
        }
    }
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
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            config("#!/bin/sh\n").control_file(
                "templates",
                TestFile::new("Template: example/foo\nType: string\n"),
            ),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("config", TestFile::symlink("missing")),
            config("#!/bin/sh\n").field("Depends", "dbconfig-common"),
            config("#!/bin/sh\n").field("Pre-Depends", "dbconfig-common (>= 1)"),
            config("#!/bin/sh\n").field("Provides", "dbconfig-common"),
            config("#!/bin/sh\n").package("dbconfig-common"),
            config("#!/bin/sh\n").udeb(),
        ];
        let fail = vec![
            config("#!/bin/sh\n. /usr/share/debconf/confmodule\n"),
            config("").control_file(
                "preinst",
                TestFile::new("#!/bin/sh\n. /usr/share/debconf/confmodule\n"),
            ),
            config("#!/bin/sh\n").field("Depends", "debconf"),
            config("#!/bin/sh\n").field("Depends", "dbconfig-common | other"),
            config("#!/bin/sh\n").field("Recommends", "dbconfig-common"),
            config("#!/bin/sh\n").control_file("templates", TestFile::symlink("missing")),
        ];
        BinaryPackageTester::new(NoDebconfTemplates, pass, fail).test_and_snapshot();
    }
}
