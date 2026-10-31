use std::path::{Path, PathBuf};

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::rule::BinaryPackageRule;

use super::scan;

pub struct ScriptUsesBinEnv;

declare_rule! {
    /// An installed script names `/bin/env` as its interpreter.
    ///
    /// Data files, not maintainer scripts. The interpreter is the first
    /// shebang token after a leading `/usr/bin/env` is removed, so
    /// `#!/bin/env python` and `#! /bin/env python` are `/bin/env`, while
    /// `#!/usr/bin/env python` is `python`. Extra is `/bin/env`. Paths under
    /// `usr/share/doc/`, `usr/src/`, and `usr/share/cargo/registry/` are
    /// skipped; examples are a different Tag. The Location is the path with
    /// no line number.
    ScriptUsesBinEnv,
    code = "LN0299",
    tag = "script-uses-bin-env",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ScriptUsesBinEnv {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for path in bin_env_scripts(files) {
            ctx.diagnostic(Diagnostic::warn("/bin/env").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn bin_env_scripts(files: &FileTree) -> Vec<PathBuf> {
    files
        .query()
        .kinds([EntryKind::RegularFile])
        .into_iter()
        .filter(|entry| !skipped(entry.path()))
        .filter_map(|entry| {
            let bytes = files.read(entry).ok()?;
            (scan::interpreter(&bytes).as_deref() == Some("/bin/env"))
                .then(|| entry.path().to_path_buf())
        })
        .collect()
}

/// Documentation, `/usr/src`, and vendored Cargo sources are not this Tag.
fn skipped(path: &Path) -> bool {
    let Some(name) = path.to_str() else {
        return true;
    };
    name.starts_with("usr/share/doc/")
        || name.starts_with("usr/src/")
        || name.starts_with("usr/share/cargo/registry/")
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn data(path: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(content))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            data("usr/bin/tool", "#!/usr/bin/env python\nprint(1)\n"),
            data("usr/bin/tool", "#!/bin/sh\necho hi\n"),
            data("usr/bin/tool", "no shebang\n"),
            data("usr/bin/tool", "#!#!#!\n"),
            data("usr/bin/tool", "#!/usr/bin/env\n"),
            data(
                "usr/share/doc/scripts-interpreters/examples/bin-env",
                "#!/bin/env perl\n",
            ),
            data("usr/share/doc/scripts/envfoo", "#! /bin/env python\n"),
            data("usr/src/pkg/tool", "#!/bin/env perl\n"),
            data("usr/share/cargo/registry/foo/tool", "#!/bin/env python\n"),
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new())
                .control_file("postinst", TestFile::new("#!/bin/env perl\n")),
        ];
        let fail = vec![
            data(
                "usr/bin/bin-env",
                "#!/bin/env perl\nprint \"Hello world\\n\";\n",
            ),
            data(
                "usr/bin/envfoo",
                "#! /bin/env python\n\nif __name__ == '__main__':\n\tprint 'Hi there'\n",
            ),
            data("usr/bin/tool", "#!/usr/bin/env /bin/env\n"),
            data("usr/lib/cgi-bin/tool", "#!/bin/env python\n").udeb(),
        ];
        BinaryPackageTester::new(ScriptUsesBinEnv, pass, fail).test_and_snapshot();
    }
}
