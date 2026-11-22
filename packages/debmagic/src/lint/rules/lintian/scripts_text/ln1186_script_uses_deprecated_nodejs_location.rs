use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::interpreter::Hashbang;
use crate::lint::rule::BinaryPackageRule;

pub struct ScriptUsesDeprecatedNodejsLocation;

declare_rule! {
    /// An installed script names `nodejs` as its interpreter.
    ///
    /// The interpreter is the first word of the hashbang after a leading
    /// `/usr/bin/env` is removed, so `#!nodejs` and `#!/usr/bin/env nodejs`
    /// are `nodejs`, while `#!/usr/bin/nodejs` is a full path and not this
    /// Tag. Extra is `nodejs`; the path is the Location. Whether the script is
    /// executable does not matter. Regular files and hardlinks under
    /// `usr/share/doc/` (other than examples), `usr/src/`, and
    /// `usr/share/cargo/registry/` are skipped; examples are a different Tag.
    /// Udebs are checked too.
    ScriptUsesDeprecatedNodejsLocation,
    code = "LN1186",
    tag = "script-uses-deprecated-nodejs-location",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for ScriptUsesDeprecatedNodejsLocation {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        for path in nodejs_scripts(files) {
            ctx.diagnostic(Diagnostic::warn("nodejs").with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

fn nodejs_scripts(files: &FileTree) -> Vec<PathBuf> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .filter_map(|entry| {
            let name = entry.path().to_str()?;
            if name.starts_with("usr/share/cargo/registry/")
                || name.starts_with("usr/src/")
                || (name.starts_with("usr/share/doc/") && !is_examples(name))
            {
                return None;
            }
            let bytes = files.read(entry).ok()?;
            if bytes.starts_with(b"#!#!#!") {
                return None;
            }
            let interpreter = Hashbang::read(&bytes)?.interpreter();
            (interpreter == "nodejs" && !is_examples(name)).then(|| entry.path().to_path_buf())
        })
        .collect()
}

/// `usr/share/doc/<package>/examples/…`.
fn is_examples(name: &str) -> bool {
    name.strip_prefix("usr/share/doc/")
        .and_then(|rest| rest.split_once('/'))
        .is_some_and(|(package, after)| !package.is_empty() && after.starts_with("examples/"))
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn script(path: &str, content: &str) -> BinaryPackageCase {
        BinaryPackageCase::from(vec![(path, TestFile::new(content).mode(0o755))])
    }

    #[test]
    fn test() {
        let pass = vec![
            BinaryPackageCase::from(Vec::<(&str, TestFile)>::new()),
            script("usr/bin/tool", "#!/usr/bin/node\n"),
            script("usr/bin/tool", "#!/usr/bin/env node\n"),
            // A full path to nodejs is not the relative name.
            script("usr/bin/tool", "#!/usr/bin/nodejs\n"),
            script("usr/bin/tool", "#!/bin/sh\n"),
            script("usr/bin/tool", "nodejs\n"),
            // Examples are another Tag.
            script("usr/share/doc/pkg/examples/tool", "#!nodejs\n"),
            // Skipped trees.
            script("usr/share/doc/pkg/tool", "#!nodejs\n"),
            script("usr/src/pkg/tool", "#!nodejs\n"),
            script("usr/share/cargo/registry/pkg/tool", "#!nodejs\n"),
        ];
        let fail = vec![
            script("usr/bin/tool", "#!nodejs\n"),
            script("usr/bin/tool", "#!/usr/bin/env nodejs\n"),
            script("usr/bin/tool", "#!/usr/bin/env nodejs\n").udeb(),
            // Not executable is still reported.
            BinaryPackageCase::from(vec![(
                "usr/bin/tool",
                TestFile::new("#!/usr/bin/env nodejs\n"),
            )]),
            BinaryPackageCase::from(vec![(
                "usr/bin/tool",
                TestFile::new("#!nodejs\n").mode(0o755).hardlink(),
            )]),
        ];
        BinaryPackageTester::new(ScriptUsesDeprecatedNodejsLocation, pass, fail)
            .test_and_snapshot();
    }
}
