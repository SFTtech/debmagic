use std::sync::LazyLock;

use regex::Regex;

use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::links::normalised_absolute_target;

pub struct SymlinkTargetInBuildTree;

declare_rule! {
    /// An absolute symlink points into a common build directory, such as
    /// `/var/lib/sbuild/`, which usually means a build path was stripped
    /// without regex quoting.
    ///
    /// The target is normalised as lintian does (`//+` to `/`, one trailing
    /// `/` dropped) and judged without its leading `/`, against the
    /// `files/build-path-regex` patterns (anchored at the start). A pattern
    /// that matches fires once. Relative targets are not judged. Udebs are
    /// checked too. Extra is the normalised target; the Location is the
    /// symlink. Error severity, from the `.tag` file.
    SymlinkTargetInBuildTree,
    code = "LN1265",
    tag = "symlink-target-in-build-tree",
    default_selected = true,
    experimental = false,
}

/// lintian 2.144.0 `data/files/build-path-regex`, vendored in `src/lint/data`.
static BUILD_PATH: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    crate::lint::data::lines(include_str!("../../../data/files/build-path-regex"))
        .map(|pattern| Regex::new(pattern).expect("valid regex"))
        .collect()
});

impl BinaryPackageRule for SymlinkTargetInBuildTree {
    fn run(&self, ctx: &mut BinaryPackageContext<'_>) {
        let Some(files) = ctx.data_files() else {
            return;
        };
        let mut found = Vec::new();
        for entry in files.entries() {
            if entry.kind() != EntryKind::Symlink {
                continue;
            }
            let Some(target) = entry.link_target() else {
                continue;
            };
            let link = target.to_string_lossy();
            let Some((normalised, rest)) = normalised_absolute_target(&link) else {
                continue;
            };
            let hits = BUILD_PATH
                .iter()
                .filter(|pattern| pattern.is_match(&rest))
                .count();
            for _ in 0..hits {
                found.push((entry.path().to_path_buf(), normalised.clone()));
            }
        }
        for (path, normalised) in found {
            ctx.diagnostic(Diagnostic::error(normalised).with_location(Location {
                path,
                line: None,
                column: None,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::tester::{BinaryPackageCase, BinaryPackageTester, TestFile};

    use super::*;

    fn links(links: &[(&str, &str)]) -> BinaryPackageCase {
        BinaryPackageCase::from(
            links
                .iter()
                .map(|(name, target)| (name.to_string(), TestFile::symlink(*target)))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn binary() {
        let pass = vec![
            links(&[]),
            // Relative targets are not judged.
            links(&[("usr/lib/x/link", "../../var/lib/sbuild/1024")]),
            links(&[("usr/bin/tool", "/usr/lib/tool")]),
            // The patterns are anchored at the start of the target.
            links(&[("usr/lib/x/link", "/usr/build/x")]),
            // A trailing slash is dropped first, leaving `build` (no `build/`).
            links(&[("usr/lib/x/link", "/build/")]),
            // A udeb is checked too, but this target matches nothing.
            BinaryPackageCase::from(vec![(
                "usr/lib/x/link",
                TestFile::symlink("/usr/lib/other"),
            )])
            .udeb(),
        ];
        let fail = vec![
            links(&[("etc/some.conf", "/var/lib/sbuild/1024")]),
            links(&[("usr/lib/x/link", "/build/tool")]),
            links(&[("usr/lib/x/link", "//var/cache/pbuilder/build/x")]),
            links(&[("usr/lib/x/link", "/tmp/buildd/x")]),
        ];
        BinaryPackageTester::new(SymlinkTargetInBuildTree, pass, fail).test_and_snapshot();
    }
}
