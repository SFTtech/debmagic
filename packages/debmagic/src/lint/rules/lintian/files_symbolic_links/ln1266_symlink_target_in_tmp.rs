use crate::declare_rule;
use crate::lint::context::BinaryPackageContext;
use crate::lint::diagnostic::{Diagnostic, Location};
use crate::lint::file_tree::EntryKind;
use crate::lint::rule::BinaryPackageRule;

use super::links::normalised_absolute_target;

pub struct SymlinkTargetInTmp;

declare_rule! {
    /// An absolute symlink points into `/tmp`, `/var/tmp`, `/usr/tmp` or
    /// `/dev/shm`. The File Hierarchy Standard says such files may be removed
    /// and must not be relied on, so shipping links into them is an error.
    ///
    /// The target is normalised as lintian does (`//+` to `/`, one trailing
    /// `/` dropped) and then matched without its leading `/`: `tmp/<something>`,
    /// `var/tmp/<something>`, `usr/tmp/<something>` or `dev/shm/<something>`,
    /// where the `<something>` is at least one character. Relative targets are
    /// not judged. Udebs are checked too. Extra is the normalised target; the
    /// Location is the symlink. Error severity, from the `.tag` file.
    SymlinkTargetInTmp,
    code = "LN1266",
    tag = "symlink-target-in-tmp",
    default_selected = true,
    experimental = false,
}

/// lintian's `is_tmp_path`, on the target without its leading `/`.
fn is_tmp_path(path: &str) -> bool {
    ["tmp/", "var/tmp/", "usr/tmp/", "dev/shm/"]
        .iter()
        .any(|prefix| {
            path.strip_prefix(prefix)
                .is_some_and(|rest| !rest.is_empty())
        })
}

impl BinaryPackageRule for SymlinkTargetInTmp {
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
            if is_tmp_path(&rest) {
                found.push((entry.path().to_path_buf(), normalised));
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
            links(&[("usr/lib/x/link", "../../tmp/x")]),
            links(&[("usr/lib/x/link", "/usr/lib/tmp/x")]),
            // Only the directory itself, with nothing below it, is not a target.
            links(&[("usr/lib/x/link", "/tmp/")]),
            links(&[("usr/lib/x/link", "/tmpfoo/x")]),
            // A udeb is checked too, but this target is fine.
            BinaryPackageCase::from(vec![(
                "usr/lib/x/link",
                TestFile::symlink("/usr/lib/other"),
            )])
            .udeb(),
        ];
        let fail = vec![
            links(&[("usr/lib/x/link", "/tmp/does-not-exist")]),
            links(&[("usr/lib/x/link", "/var/tmp/x")]),
            links(&[("usr/lib/x/link", "/usr/tmp/x")]),
            links(&[("usr/lib/x/link", "/dev/shm/x")]),
            // Double slashes are normalised before the check.
            links(&[("usr/lib/x/link", "//tmp//x")]),
        ];
        BinaryPackageTester::new(SymlinkTargetInTmp, pass, fail).test_and_snapshot();
    }
}
