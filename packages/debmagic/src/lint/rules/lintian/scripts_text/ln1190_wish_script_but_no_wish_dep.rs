use std::path::PathBuf;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::debian_control::ControlParagraph;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::{EntryKind, FileTree};
use crate::lint::interpreter::{self, Hashbang};
use crate::lint::package_relation::PackageRelation;
use crate::lint::rule::BinaryPackageRule;

pub struct WishScriptButNoWishDep;

declare_rule! {
    /// An executable script runs `wish` without a dependency that provides it.
    ///
    /// The interpreter basename is `wish`, as in `/usr/bin/wish` or
    /// `/usr/bin/env wish`. The relation is the `wish` family of lintian's
    /// `scripts/versioned-interpreters` table: `wish | tk`, then `tk8.3`
    /// through `tk8.6`. The package's `Pre-Depends`, `Depends`,
    /// `Recommends`, `Suggests`, `Provides`, and package name must satisfy
    /// that relation. Extra is the interpreter and
    /// `(does not satisfy wish | tk | tk8.3 | …)`; the path is the Location.
    /// Only executable regular files and hardlinks are checked. Paths under
    /// `usr/share/doc/`, `usr/src/`, and `usr/share/cargo/registry/` are
    /// skipped. Udebs are checked too.
    WishScriptButNoWishDep,
    code = "LN1190",
    tag = "wish-script-but-no-wish-dep",
    default_selected = true,
    experimental = false,
}

impl BinaryPackageRule for WishScriptButNoWishDep {
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

/// Executable scripts that run `wish` and whose package does not provide it.
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
            if interpreter::basename(&interpreter) != "wish" {
                return None;
            }
            let known = interpreter::lookup("wish")?;
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
            // Satisfied by any alternative of the family, or by Provides or the name.
            script("usr/bin/tool", "#!/usr/bin/wish\n").field("Depends", "tk8.6"),
            script("usr/bin/tool", "#!/usr/bin/wish\n").field("Depends", "wish"),
            script("usr/bin/tool", "#!/usr/bin/wish\n").field("Recommends", "tk"),
            script("usr/bin/tool", "#!/usr/bin/wish\n").field("Provides", "wish"),
            script("usr/bin/tool", "#!/usr/bin/wish\n").package("wish"),
            // Not executable, not a script, or other interpreters.
            BinaryPackageCase::from(vec![("usr/bin/tool", TestFile::new("#!/usr/bin/wish\n"))]),
            script("usr/bin/tool", "wish\n"),
            script("usr/bin/tool", "#!/usr/bin/tclsh\n").field("Depends", "tcl8.6"),
            script("usr/bin/tool", "#!/bin/sh\n"),
            // Skipped trees.
            script("usr/share/doc/pkg/tool", "#!/usr/bin/wish\n"),
            script("usr/src/pkg/tool", "#!/usr/bin/wish\n"),
            script("usr/share/cargo/registry/pkg/tool", "#!/usr/bin/wish\n"),
        ];
        let fail = vec![
            script("usr/bin/tool", "#!/usr/bin/wish\n"),
            script("usr/bin/tool", "#!/usr/bin/env wish\n"),
            script("usr/bin/tool", "#!/usr/bin/wish\n").field("Depends", "python3"),
            script("usr/bin/tool", "#!/usr/bin/wish\n").udeb(),
            BinaryPackageCase::from(vec![(
                "usr/bin/tool",
                TestFile::new("#!/usr/bin/wish\n").mode(0o755).hardlink(),
            )]),
        ];
        BinaryPackageTester::new(WishScriptButNoWishDep, pass, fail).test_and_snapshot();
    }
}
