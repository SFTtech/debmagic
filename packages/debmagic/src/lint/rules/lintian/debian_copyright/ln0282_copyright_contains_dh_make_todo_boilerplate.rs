use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::Diagnostic;
use crate::lint::rule::BinaryPackageRule;

use super::copyright_text;

pub struct CopyrightContainsDhMakeTodoBoilerplate;

declare_rule! {
    /// The copyright file still has a dh-make TODO paragraph.
    ///
    /// The first paragraph is matched literally. The second is matched as a
    /// regular expression, so `*` in `/debian/*` quantifies the slash and the
    /// literal asterisk does not match. Udebs are not checked. Extra and
    /// Location are empty.
    CopyrightContainsDhMakeTodoBoilerplate,
    code = "LN0282",
    tag = "copyright-contains-dh_make-todo-boilerplate",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for CopyrightContainsDhMakeTodoBoilerplate {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(contents) = copyright_text::installed_text(ctx) else {
            return;
        };
        if copyright_text::contains_dh_make_todo(&contents) {
            ctx.diagnostic(Diagnostic::error(""));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn installed(body: &str) -> Vec<(&'static str, TestFile)> {
        vec![
            ("usr/share/doc/example", TestFile::directory()),
            ("usr/share/doc/example/copyright", TestFile::new(body)),
        ]
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(installed("Copyright 2024 Someone\n")),
            // `*` quantifies the slash, so the literal asterisk is not the pattern.
            BinaryPackageCase::from(installed(
                "# If you want to use GPL v2 or later for the /debian/* files use\n# the following clauses, or change it to suit. Delete these two lines\n",
            )),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(installed(
                "# Please also look if there are files or directories which have a\n# different copyright/license attached and list them here.\n",
            ))
            .udeb(),
        ];
        let fail = vec![
            BinaryPackageCase::from(installed(
                "Copyright (C) 2004 Someone\n\n# Please also look if there are files or directories which have a\n# different copyright/license attached and list them here.\n",
            )),
            BinaryPackageCase::from(installed(
                "# If you want to use GPL v2 or later for the /debian/ files use\n# the following clauses, or change it to suit. Delete these two lines\n",
            )),
        ];
        BinaryPackageTester::new(CopyrightContainsDhMakeTodoBoilerplate, pass, fail)
            .test_and_snapshot();
    }
}
