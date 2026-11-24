use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::interpreter::{self, Hashbang};
use crate::lint::package_relation::PackageRelation;
use crate::lint::rule::BinaryPackageRule;

pub struct GawkScriptButNoGawkDep;

declare_rule! {
    /// An executable script runs `gawk` without a dependency that provides it.
    ///
    /// The interpreter basename is `gawk`, as in `/usr/bin/gawk` or
    /// `/usr/bin/env gawk`. The relation is the `gawk` entry of lintian's
    /// `scripts/interpreters` table, and the package's `Pre-Depends`,
    /// `Depends`, `Recommends`, `Suggests`, `Provides`, and package name must
    /// imply it. Extra is the interpreter and `(does not satisfy gawk)`; the
    /// path is the Location. Executable regular files and hardlinks under
    /// `usr/share/doc/`, `usr/src/`, and `usr/share/cargo/registry/` are
    /// skipped. Udebs are checked too.
    GawkScriptButNoGawkDep,
    code = "LN0983",
    tag = "gawk-script-but-no-gawk-dep",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for GawkScriptButNoGawkDep {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let package_name = ctx.package_name().to_string();
        let prerequisites = all_prerequisites(ctx.installation_control().fields(), &package_name);
        let Some(files) = ctx.data_files() else {
            return;
        };
        for (path, extra) in missing(files, &prerequisites) {
            ctx.diagnostic(Diagnostic::error(extra).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

/// lintian's `all_prerequisites`: `relation('all')` AND `Provides` AND the
/// package's own name.
fn all_prerequisites(fields: &ControlParagraph, package_name: &str) -> PackageRelation {
    fields
        .relations_and(&["Pre-Depends", "Depends", "Recommends", "Suggests"])
        .and(&fields.relation("Provides").unwrap_or_default())
        .and(&PackageRelation::parse(package_name))
}

/// Executable scripts that run `gawk` and whose package does not provide it.
fn missing(files: &FileTree, prerequisites: &PackageRelation) -> Vec<(PathBuf, String)> {
    files
        .query()
        .kinds([EntryKind::RegularFile, EntryKind::Hardlink])
        .into_iter()
        .filter(|entry| entry.is_executable_file())
        .filter_map(|entry| {
            let name = entry.path().to_str()?;
            if name.starts_with("usr/share/doc/")
                || name.starts_with("usr/src/")
                || name.starts_with("usr/share/cargo/registry/")
            {
                return None;
            }
            let bytes = files.read(entry).ok()?;
            let interpreter = Hashbang::read(&bytes)?.interpreter();
            if interpreter::basename(&interpreter) != "gawk" {
                return None;
            }
            let known = interpreter::lookup("gawk")?;
            if known.prerequisites.is_empty() || prerequisites.satisfies(&known.prerequisites) {
                return None;
            }
            Some((
                entry.path().to_path_buf(),
                format!("{interpreter} (does not satisfy {})", known.prerequisites),
            ))
        })
        .collect()
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
            // Satisfied by Depends, Recommends, Provides, or the package name.
            script("usr/bin/tool", "#!/usr/bin/gawk -f\n").field("Depends", "gawk"),
            script("usr/bin/tool", "#!/usr/bin/gawk\n").field("Recommends", "gawk"),
            script("usr/bin/tool", "#!/usr/bin/gawk\n").field("Provides", "gawk"),
            script("usr/bin/tool", "#!/usr/bin/gawk\n").package("gawk"),
            // Not executable, not a script, or not a file with a hashbang.
            BinaryPackageCase::from(vec![("usr/bin/tool", TestFile::new("#!/usr/bin/gawk\n"))]),
            script("usr/bin/tool", "gawk\n"),
            script("usr/bin/tool", "#!\n"),
            // Other interpreters are other Tags.
            script("usr/bin/tool", "#!/bin/sh\n"),
            script("usr/bin/tool", "#!/usr/bin/mawk -f\n"),
            // Skipped trees.
            script("usr/share/doc/pkg/examples/tool", "#!/usr/bin/gawk\n"),
            script("usr/src/pkg/tool", "#!/usr/bin/gawk\n"),
            script("usr/share/cargo/registry/pkg/tool", "#!/usr/bin/gawk\n"),
        ];
        let fail = vec![
            script("usr/bin/tool", "#!/usr/bin/gawk -f\n"),
            script("usr/bin/tool", "#!/usr/bin/env gawk\n"),
            script("usr/bin/tool", "#!/usr/bin/gawk\n").field("Depends", "mawk"),
            script("usr/bin/tool", "#!/usr/bin/gawk\n").udeb(),
            BinaryPackageCase::from(vec![(
                "usr/bin/tool",
                TestFile::new("#!/usr/bin/gawk\n").mode(0o755).hardlink(),
            )]),
        ];
        BinaryPackageTester::new(GawkScriptButNoGawkDep, pass, fail).test_and_snapshot();
    }
}
