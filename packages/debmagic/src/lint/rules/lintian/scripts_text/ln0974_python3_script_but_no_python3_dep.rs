use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::rule::BinaryPackageRule;

use super::interpreter_hits::{installed, installed_prerequisites};

pub struct Python3ScriptButNoPython3Dep;

declare_rule! {
    /// An executable installed script names `python3` as its interpreter, and
    /// the package does not satisfy `python3:any | python3-minimal:any`.
    ///
    /// The package's relations are Pre-Depends, Depends, Recommends, Suggests,
    /// Provides and its own name. Extra is the interpreter and the dependency
    /// it lacks. Skipped: `usr/share/doc/` (examples too), `usr/src/`,
    /// `usr/share/cargo/registry/`, a Python
    /// script under `usr/lib/python3/dist-packages/`, a non-executable file
    /// outside a `bin/` directory with no absolute interpreter, and a `.in`
    /// file whose interpreter is a build placeholder such as `@PERL@`. Udebs
    /// are checked. The Location is the path with no line number.
    Python3ScriptButNoPython3Dep,
    code = "LN0974",
    tag = "python3-script-but-no-python3-dep",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for Python3ScriptButNoPython3Dep {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let prerequisites = installed_prerequisites(ctx);
        let hits = match ctx.data_files() {
            Some(files) => installed(files, &prerequisites),
            None => return,
        };
        for hit in hits
            .into_iter()
            .filter(|hit| hit.tag == "python3-script-but-no-python3-dep")
        {
            ctx.diagnostic(Diagnostic::error(hit.extra).with_location(Location {
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

    fn script(path: &str, content: &str, mode: u32) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(content).mode(mode))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            script("usr/bin/tool", "#!/usr/bin/python3\nprint(1)\n", 0o644),
            script("usr/share/tool/tool", "#!/usr/bin/python3\n", 0o644),
            script("usr/share/doc/pkg/tool", "#!/usr/bin/python3\n", 0o755),
            script("usr/src/pkg/tool", "#!/usr/bin/python3\n", 0o755),
            script(
                "usr/lib/python3/dist-packages/tool",
                "#!/usr/bin/python\n",
                0o755,
            ),
            script("usr/bin/tool", "#!/bin/sh\necho\n", 0o755),
            script("usr/bin/tool", "no shebang\n", 0o755),
            script("usr/share/tool/tool.in", "#!@PYTHON3@\n", 0o755),
            BinaryPackageCase::from(vec![(
                "usr/bin/tool",
                TestFile::new("#!/usr/bin/python3\n").mode(0o755),
            )])
            .field("Depends", "python3"),
        ];
        let fail = vec![
            script("usr/bin/tool", "#!/usr/bin/python3\nprint(1)\n", 0o755),
            script("usr/bin/tool", "#!/usr/bin/env python3\nprint(1)\n", 0o755),
            script("usr/bin/tool", "#!/usr/bin/python3 -u\nprint(1)\n", 0o755).udeb(),
            script("usr/lib/tool", "#!/usr/bin/python3\n", 0o755),
            script("usr/share/tool/tool.in", "#!/usr/bin/python3\n", 0o755),
        ];
        BinaryPackageTester::new(Python3ScriptButNoPython3Dep, pass, fail).test_and_snapshot();
    }

    /// lintian's Scripts check (`visit_installed_files`) returns before any
    /// interpreter check for every path under `usr/share/cargo/registry/`,
    /// so upstream source shipped there is not checked for a python3 dependency.
    #[test]
    fn cargo_registry_scripts_are_not_checked() {
        let pass = vec![
            script(
                "usr/share/cargo/registry/pkg-1.0/tool.py",
                "#!/usr/bin/python3\n",
                0o755,
            ),
            script(
                "usr/share/cargo/registry/pkg-1.0/c/test.py",
                "#!/usr/bin/env python3\n",
                0o755,
            ),
        ];
        // Boundaries: the same script outside the registry tree is reported,
        // and so is one in a sibling directory of `registry/`.
        let fail = vec![
            script("usr/share/tool/tool.py", "#!/usr/bin/python3\n", 0o755),
            script("usr/share/cargo/tool.py", "#!/usr/bin/python3\n", 0o755),
        ];
        BinaryPackageTester::new(Python3ScriptButNoPython3Dep, pass, fail)
            .test_and_snapshot_with_suffix("cargo_registry");
    }
}
