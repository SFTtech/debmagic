use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::interpreter_hits::{control, control_facts};

pub struct MultiArchSamePackageCallsPycompile;

declare_rule! {
    /// A package with `Multi-Arch: same` runs `pycompile` or `py3compile` in a
    /// shell maintainer script.
    ///
    /// The line must start with the command (after blanks), and the command
    /// is matched after comments are removed and `\` continuations are joined.
    /// The Location is the script and the physical line where the command's
    /// logical line ends; the extra is empty. Only shell scripts are read, and
    /// only those whose interpreter is known to lintian (`/usr/local/`, `sh`,
    /// `bash`, `perl`, `config`, `postrm`, or an unversioned table entry).
    /// Udebs are checked.
    MultiArchSamePackageCallsPycompile,
    code = "LN0977",
    tag = "multi-arch-same-package-calls-pycompile",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for MultiArchSamePackageCallsPycompile {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let facts = control_facts(ctx);
        let hits = match ctx.control_files() {
            Some(files) => control(files, &facts),
            None => return,
        };
        for hit in hits
            .into_iter()
            .filter(|hit| hit.tag == "multi-arch-same-package-calls-pycompile")
        {
            ctx.diagnostic(Diagnostic::warn(hit.extra).with_location(Location {
                path: hit.path,
                line: hit.line,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn postinst(content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
            .control_file("postinst", TestFile::new(content))
            .field("Multi-Arch", "same")
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::new("#!/bin/sh\npycompile || true\n"))
                .field("Multi-Arch", "foreign"),
            postinst("#!/bin/sh\n# pycompile\nmypycompile || true\n"),
            postinst("#!/usr/bin/perl\npycompile\n"),
            postinst("#!/bin/sh\necho pycompile\n"),
            postinst("#!/usr/bin/python3\npycompile\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("preinst", TestFile::new("#!/bin/sh\n"))
                .control_file("postinst", TestFile::new("#!/bin/sh\n"))
                .field("Multi-Arch", "same"),
        ];
        let fail = vec![
            postinst(
                "#!/bin/sh\n\nset -e\n\n#DEBHELPER#\n\npycompile || true\npy3compile || true\n",
            ),
            postinst("#!/bin/sh\n  py3compile -V 3.8\n").udeb(),
            postinst("#!/bin/sh\npy3compile \\\n  --foo\n"),
        ];
        BinaryPackageTester::new(MultiArchSamePackageCallsPycompile, pass, fail)
            .test_and_snapshot();
    }
}
